use serde_json::{json, Value};

pub(crate) fn transition_kind_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["crossfade", "dipToBlack", "dipToWhite", "wipe"]
    })
}

pub(crate) fn add_transition_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackId", "transition"],
        "additionalProperties": false,
        "properties": {
            "type": { "type": "string", "enum": ["addTransition"] },
            "trackId": { "type": "string", "minLength": 1 },
            "transition": {
                "type": "object",
                "required": ["id", "leftItemId", "rightItemId", "kind", "durationSeconds"],
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "leftItemId": { "type": "string", "minLength": 1 },
                    "rightItemId": { "type": "string", "minLength": 1 },
                    "kind": transition_kind_schema(),
                    "durationSeconds": { "type": "number", "exclusiveMinimum": 0, "maximum": 5 }
                }
            }
        }
    })
}

pub(crate) fn update_transition_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackId", "transitionId", "kind", "durationSeconds"],
        "additionalProperties": false,
        "properties": {
            "type": { "type": "string", "enum": ["updateTransition"] },
            "trackId": { "type": "string", "minLength": 1 },
            "transitionId": { "type": "string", "minLength": 1 },
            "kind": {
                "anyOf": [
                    transition_kind_schema(),
                    { "type": "null" }
                ]
            },
            "durationSeconds": { "type": ["number", "null"], "exclusiveMinimum": 0, "maximum": 5 }
        }
    })
}

pub(crate) fn remove_transition_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackId", "transitionId"],
        "additionalProperties": false,
        "properties": {
            "type": { "type": "string", "enum": ["removeTransition"] },
            "trackId": { "type": "string", "minLength": 1 },
            "transitionId": { "type": "string", "minLength": 1 }
        }
    })
}
