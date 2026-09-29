use serde_json::{json, Value};

pub(crate) fn assign_media_folder_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "mediaId", "folderId"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["assignMediaFolder"]
            },
            "mediaId": { "type": "string", "minLength": 1 },
            "folderId": {
                "anyOf": [
                    { "type": "string", "minLength": 1 },
                    { "type": "null" }
                ]
            }
        }
    })
}

pub(crate) fn create_media_folder_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "folder"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["createMediaFolder"]
            },
            "folder": {
                "type": "object",
                "required": ["id", "name", "parentId"],
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "parentId": {
                        "anyOf": [
                            { "type": "string", "minLength": 1 },
                            { "type": "null" }
                        ]
                    }
                }
            }
        }
    })
}

pub(crate) fn rename_media_folder_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "folderId", "name"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["renameMediaFolder"]
            },
            "folderId": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn delete_media_folder_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "folderId"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["deleteMediaFolder"]
            },
            "folderId": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn rename_media_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "mediaId", "name"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["renameMedia"]
            },
            "mediaId": { "type": "string", "minLength": 1 },
            "name": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn delete_media_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "mediaIds"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["deleteMedia"]
            },
            "mediaIds": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string", "minLength": 1 }
            }
        }
    })
}

pub(crate) fn remove_tracks_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackIds"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["removeTracks"]
            },
            "trackIds": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string", "minLength": 1 }
            }
        }
    })
}

pub(crate) fn update_render_settings_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "settings"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateRenderSettings"]
            },
            "settings": {
                "type": "object",
                "required": ["width", "height", "fps", "loudnessLufs", "captions"],
                "additionalProperties": false,
                "properties": {
                    "width": { "type": "integer", "minimum": 1 },
                    "height": { "type": "integer", "minimum": 1 },
                    "fps": { "type": "number", "exclusiveMinimum": 0 },
                    "loudnessLufs": { "type": "number" },
                    "captions": { "type": "string", "enum": ["burn_in", "mux", "off"] }
                }
            }
        }
    })
}
