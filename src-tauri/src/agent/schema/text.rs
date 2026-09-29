use super::*;
use serde_json::{json, Value};

pub(crate) fn caption_repair_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "captionItemId",
            "transcriptId",
            "wordIndex",
            "text",
            "startSeconds",
            "endSeconds",
            "repairId",
            "createdAt"
        ],
        "additionalProperties": false,
        "properties": {
            "captionItemId": { "type": "string", "minLength": 1 },
            "transcriptId": { "type": "string", "minLength": 1 },
            "wordIndex": { "type": "integer", "minimum": 0 },
            "text": { "type": "string", "minLength": 1 },
            "startSeconds": { "type": "number", "minimum": 0 },
            "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
            "repairId": { "type": "string", "minLength": 1 },
            "createdAt": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn apply_caption_repair_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "repair"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["applyCaptionRepair"]
            },
            "repair": caption_repair_schema()
        }
    })
}

pub(crate) fn transcript_word_edit_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "transcriptId",
            "wordIndex",
            "text",
            "startSeconds",
            "endSeconds",
            "repairId",
            "createdAt"
        ],
        "additionalProperties": false,
        "properties": {
            "transcriptId": { "type": "string", "minLength": 1 },
            "wordIndex": { "type": "integer", "minimum": 0 },
            "text": nullable_non_empty_string_schema(),
            "startSeconds": nullable_non_negative_number_schema(),
            "endSeconds": nullable_positive_number_schema(),
            "repairId": { "type": "string", "minLength": 1 },
            "createdAt": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn edit_transcript_words_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "edits"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["editTranscriptWords"]
            },
            "edits": {
                "type": "array",
                "minItems": 1,
                "items": transcript_word_edit_schema()
            }
        }
    })
}

pub(crate) fn edit_caption_text_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "text"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["editCaptionText"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "text": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn edit_text_item_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "text"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["editTextItem"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "text": { "type": "string", "minLength": 1 }
        }
    })
}

pub(crate) fn update_template_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "updates"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateTemplateItems"]
            },
            "updates": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["itemId", "startSeconds", "durationSeconds", "templateFields"],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
                        "templateFields": {
                            "type": "object",
                            "required": ["headline", "subline", "logoAssetId"],
                            "additionalProperties": false,
                            "properties": {
                                "headline": { "type": ["string", "null"] },
                                "subline": { "type": ["string", "null"] },
                                "logoAssetId": { "type": ["string", "null"] }
                            }
                        }
                    }
                }
            }
        }
    })
}

pub(crate) fn update_template_override_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "override"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateTemplateOverride"]
            },
            "override": {
                "type": "object",
                "required": [
                    "templateId",
                    "name",
                    "fields",
                    "style",
                    "visualTreatment",
                    "motion",
                    "safeZone",
                    "avoid"
                ],
                "additionalProperties": false,
                "properties": {
                    "templateId": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 },
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
                    "style": {
                        "type": "object",
                        "required": ["accentColor", "backgroundColor"],
                        "additionalProperties": false,
                        "properties": {
                            "accentColor": { "type": ["string", "null"] },
                            "backgroundColor": { "type": ["string", "null"] }
                        }
                    },
                    "visualTreatment": { "type": "string", "minLength": 1 },
                    "motion": { "type": "string", "minLength": 1 },
                    "safeZone": { "type": "string", "minLength": 1 },
                    "avoid": { "type": "string", "minLength": 1 }
                }
            }
        }
    })
}

pub(crate) fn update_text_overlay_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "updates"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateTextOverlayItems"]
            },
            "updates": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": [
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
                    ],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
                        "text": { "type": "string", "minLength": 1 },
                        "visualTreatment": { "type": "string", "minLength": 1 },
                        "motion": { "type": "string", "minLength": 1 },
                        "safeZone": { "type": "string", "minLength": 1 },
                        "avoid": { "type": "string", "minLength": 1 },
                        "fontName": { "type": ["string", "null"], "minLength": 1 },
                        "fontSize": { "type": ["number", "null"], "exclusiveMinimum": 0 },
                        "color": { "type": ["string", "null"], "minLength": 1 },
                        "alignment": { "type": ["string", "null"], "minLength": 1 }
                    }
                }
            }
        }
    })
}
