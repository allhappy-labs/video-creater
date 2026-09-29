use super::*;
use serde_json::{json, Value};

pub(crate) fn create_track_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "track", "afterTrackId"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["createTrack"]
            },
            "track": {
                "type": "object",
                "required": ["id", "name", "kind", "locked", "enabled", "items"],
                "additionalProperties": false,
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "name": { "type": "string", "minLength": 1 },
                    "kind": {
                        "type": "string",
                        "enum": ["video", "hyperframe_scene", "overlay", "caption", "audio"]
                    },
                    "locked": { "type": "boolean" },
                    "enabled": { "type": "boolean" },
                    "items": {
                        "type": "array",
                        "maxItems": 0,
                        "items": {
                            "type": "object",
                            "required": [],
                            "additionalProperties": false,
                            "properties": {}
                        }
                    }
                }
            },
            "afterTrackId": nullable_non_empty_string_schema()
        }
    })
}

pub(crate) fn add_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "targetTrackId", "items"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["addItems"]
            },
            "targetTrackId": { "type": "string", "minLength": 1 },
            "items": {
                "type": "array",
                "minItems": 1,
                "items": timeline_item_schema()
            }
        }
    })
}

pub(crate) fn insert_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "targetTrackId", "insertSeconds", "items"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["insertItems"]
            },
            "targetTrackId": { "type": "string", "minLength": 1 },
            "insertSeconds": { "type": "number", "minimum": 0 },
            "items": {
                "type": "array",
                "minItems": 1,
                "items": timeline_item_schema()
            }
        }
    })
}

pub(crate) fn timeline_item_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "id",
            "kind",
            "startSeconds",
            "durationSeconds",
            "source",
            "label",
            "properties"
        ],
        "additionalProperties": false,
        "properties": {
            "id": { "type": "string", "minLength": 1 },
            "kind": {
                "type": "string",
                "enum": ["video_clip", "hyperframe_scene", "overlay", "caption", "audio_clip"]
            },
            "startSeconds": { "type": "number", "minimum": 0 },
            "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
            "source": timeline_source_schema(),
            "label": { "type": "string", "minLength": 1 },
            "properties": timeline_item_properties_schema()
        }
    })
}

pub(crate) fn timeline_item_properties_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "sourceIn",
            "sourceOut",
            "generatedEdit",
            "generatedAssetId",
            "generatedOutputMediaId",
            "reason",
            "text",
            "textEdited",
            "stylePreset",
            "templateId",
            "templateFields",
            "motionPresetId",
            "visualTreatment",
            "motion",
            "safeZone",
            "avoid",
            "fadeInSeconds",
            "fadeOutSeconds",
            "volumeDb"
        ],
        "additionalProperties": false,
        "properties": {
            "sourceIn": nullable_non_negative_number_schema(),
            "sourceOut": nullable_positive_number_schema(),
            "generatedEdit": { "type": ["boolean", "null"] },
            "generatedAssetId": nullable_non_empty_string_schema(),
            "generatedOutputMediaId": nullable_non_empty_string_schema(),
            "reason": nullable_non_empty_string_schema(),
            "text": nullable_non_empty_string_schema(),
            "textEdited": { "type": ["boolean", "null"] },
            "stylePreset": nullable_non_empty_string_schema(),
            "templateId": nullable_non_empty_string_schema(),
            "templateFields": {
                "type": "object",
                "required": ["headline", "subline", "logoAssetId"],
                "additionalProperties": false,
                "properties": {
                    "headline": { "type": ["string", "null"] },
                    "subline": { "type": ["string", "null"] },
                    "logoAssetId": { "type": ["string", "null"] }
                }
            },
            "motionPresetId": nullable_non_empty_string_schema(),
            "visualTreatment": nullable_non_empty_string_schema(),
            "motion": nullable_non_empty_string_schema(),
            "safeZone": nullable_non_empty_string_schema(),
            "avoid": nullable_non_empty_string_schema(),
            "fadeInSeconds": nullable_non_negative_number_schema(),
            "fadeOutSeconds": nullable_non_negative_number_schema(),
            "volumeDb": { "type": ["number", "null"], "minimum": -60, "maximum": 24 }
        }
    })
}

pub(crate) fn timeline_source_schema() -> Value {
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
}

pub(crate) fn remove_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemIds"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["removeItems"]
            },
            "itemIds": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string", "minLength": 1 }
            }
        }
    })
}

pub(crate) fn set_track_locked_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackId", "locked"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["setTrackLocked"]
            },
            "trackId": { "type": "string", "minLength": 1 },
            "locked": { "type": "boolean" }
        }
    })
}

pub(crate) fn set_track_sync_locked_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackId", "syncLocked"],
        "additionalProperties": false,
        "properties": {
            "type": { "type": "string", "enum": ["setTrackSyncLocked"] },
            "trackId": { "type": "string", "minLength": 1 },
            "syncLocked": { "type": "boolean" }
        }
    })
}

pub(crate) fn set_track_enabled_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trackId", "enabled"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["setTrackEnabled"]
            },
            "trackId": { "type": "string", "minLength": 1 },
            "enabled": { "type": "boolean" }
        }
    })
}

pub(crate) fn move_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "moves"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["moveItems"]
            },
            "moves": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["itemId", "targetTrackId", "startSeconds"],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "targetTrackId": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 }
                    }
                }
            }
        }
    })
}

pub(crate) fn reorder_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "reorder"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["reorderItems"]
            },
            "reorder": {
                "type": "object",
                "required": ["targetTrackId", "itemIds", "startSeconds", "gapSeconds"],
                "additionalProperties": false,
                "properties": {
                    "targetTrackId": { "type": "string", "minLength": 1 },
                    "itemIds": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string", "minLength": 1 }
                    },
                    "startSeconds": { "type": "number", "minimum": 0 },
                    "gapSeconds": { "type": "number", "minimum": 0 }
                }
            }
        }
    })
}

pub(crate) fn resize_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "resizes"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["resizeItems"]
            },
            "resizes": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["itemId", "durationSeconds"],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "durationSeconds": { "type": "number", "exclusiveMinimum": 0 }
                    }
                }
            }
        }
    })
}

pub(crate) fn trim_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "trims"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["trimItems"]
            },
            "trims": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["itemId", "startSeconds", "durationSeconds", "sourceIn", "sourceOut"],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "durationSeconds": { "type": "number", "exclusiveMinimum": 0 },
                        "sourceIn": nullable_non_negative_number_schema(),
                        "sourceOut": nullable_positive_number_schema()
                    }
                }
            }
        }
    })
}

pub(crate) fn ripple_delete_ranges_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "ranges"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["rippleDeleteRanges"]
            },
            "ranges": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["startSeconds", "endSeconds", "trackIds"],
                    "additionalProperties": false,
                    "properties": {
                        "startSeconds": { "type": "number", "minimum": 0 },
                        "endSeconds": { "type": "number", "exclusiveMinimum": 0 },
                        "trackIds": {
                            "type": "array",
                            "minItems": 1,
                            "items": { "type": "string", "minLength": 1 }
                        }
                    }
                }
            }
        }
    })
}

pub(crate) fn split_items_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "splits"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["splitItems"]
            },
            "splits": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["itemId", "newItemId", "splitSeconds"],
                    "additionalProperties": false,
                    "properties": {
                        "itemId": { "type": "string", "minLength": 1 },
                        "newItemId": { "type": "string", "minLength": 1 },
                        "splitSeconds": { "type": "number", "exclusiveMinimum": 0 }
                    }
                }
            }
        }
    })
}

pub(crate) fn update_audio_fade_out_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "fadeOutSeconds"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateAudioFadeOut"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "fadeOutSeconds": { "type": "number", "minimum": 0 }
        }
    })
}

pub(crate) fn update_audio_fades_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "fadeInSeconds", "fadeOutSeconds"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateAudioFades"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "fadeInSeconds": { "type": "number", "minimum": 0 },
            "fadeOutSeconds": { "type": "number", "minimum": 0 }
        }
    })
}

pub(crate) fn update_audio_volume_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "volumeDb"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateAudioVolume"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "volumeDb": {
                "anyOf": [
                    { "type": "number", "minimum": -60, "maximum": 24 },
                    { "type": "null" }
                ]
            }
        }
    })
}

pub(crate) fn update_audio_clip_speed_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "speed"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateAudioClipSpeed"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "speed": { "type": "number", "minimum": 0.1, "maximum": 8 }
        }
    })
}

pub(crate) fn detach_audio_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "audioItemId", "targetTrackId", "linkGroupId"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["detachAudio"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "audioItemId": { "type": "string", "minLength": 1 },
            "targetTrackId": { "type": "string", "minLength": 1 },
            "linkGroupId": { "type": "string", "minLength": 1, "maxLength": 128 }
        }
    })
}

pub(crate) fn update_clip_reverse_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "reverse"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateClipReverse"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "reverse": { "type": "boolean" }
        }
    })
}
