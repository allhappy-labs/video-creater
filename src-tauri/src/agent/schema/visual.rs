use serde_json::{json, Map, Value};

pub(crate) fn update_visual_clip_opacity_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "opacity"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateVisualClipOpacity"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "opacity": { "type": "number", "minimum": 0, "maximum": 1 }
        }
    })
}

pub(crate) fn update_visual_clip_transform_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "transform"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateVisualClipTransform"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "transform": {
                "type": "object",
                "required": [
                    "centerX",
                    "centerY",
                    "width",
                    "height",
                    "flipHorizontal",
                    "flipVertical"
                ],
                "additionalProperties": false,
                "properties": {
                    "centerX": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                    "centerY": { "type": ["number", "null"], "minimum": 0, "maximum": 1 },
                    "width": { "type": ["number", "null"], "exclusiveMinimum": 0, "maximum": 1 },
                    "height": { "type": ["number", "null"], "exclusiveMinimum": 0, "maximum": 1 },
                    "flipHorizontal": { "type": ["boolean", "null"] },
                    "flipVertical": { "type": ["boolean", "null"] }
                }
            }
        }
    })
}

pub(crate) fn update_visual_clip_crop_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "crop"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateVisualClipCrop"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "crop": {
                "type": "object",
                "required": ["cropTop", "cropRight", "cropBottom", "cropLeft"],
                "additionalProperties": false,
                "properties": {
                    "cropTop": { "type": ["number", "null"], "minimum": 0, "exclusiveMaximum": 1 },
                    "cropRight": { "type": ["number", "null"], "minimum": 0, "exclusiveMaximum": 1 },
                    "cropBottom": { "type": ["number", "null"], "minimum": 0, "exclusiveMaximum": 1 },
                    "cropLeft": { "type": ["number", "null"], "minimum": 0, "exclusiveMaximum": 1 }
                }
            }
        }
    })
}

pub(crate) fn set_item_keyframes_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemId", "property", "keyframes"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["setItemKeyframes"]
            },
            "itemId": { "type": "string", "minLength": 1 },
            "property": {
                "type": "string",
                "enum": [
                    "opacity",
                    "volumeDb",
                    "positionX",
                    "positionY",
                    "scale",
                    "scaleX",
                    "scaleY",
                    "rotationDegrees",
                    "cropTop",
                    "cropRight",
                    "cropBottom",
                    "cropLeft"
                ]
            },
            "keyframes": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["atSeconds", "value", "easing"],
                    "additionalProperties": false,
                    "properties": {
                        "atSeconds": { "type": "number", "minimum": 0 },
                        "value": { "type": "number" },
                        "easing": {
                            "anyOf": [
                                { "type": "string", "minLength": 1 },
                                { "type": "null" }
                            ]
                        }
                    }
                }
            }
        }
    })
}

pub(crate) fn update_item_effects_action_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "itemIds", "effects"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateItemEffects"]
            },
            "itemIds": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string", "minLength": 1 }
            },
            "effects": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["effectType", "enabled", "params"],
                    "additionalProperties": false,
                    "properties": {
                        "effectType": { "type": "string", "minLength": 1 },
                        "enabled": { "type": "boolean" },
                        "params": {
                            "type": "object",
                            "required": [],
                            "additionalProperties": false,
                            "properties": {}
                        }
                    }
                }
            }
        }
    })
}

pub(crate) fn update_item_color_grade_action_schema() -> Value {
    let mut grade_properties = Map::new();
    grade_properties.insert(
        "exposure".into(),
        nullable_bounded_number_schema(json!(-3), json!(3)),
    );
    grade_properties.insert(
        "contrast".into(),
        nullable_bounded_number_schema(json!(0.5), json!(1.5)),
    );
    grade_properties.insert(
        "saturation".into(),
        nullable_bounded_number_schema(json!(0), json!(2)),
    );
    grade_properties.insert(
        "temperature".into(),
        nullable_bounded_number_schema(json!(2000), json!(11000)),
    );
    grade_properties.insert(
        "tint".into(),
        nullable_bounded_number_schema(json!(-100), json!(100)),
    );
    grade_properties.insert(
        "vibrance".into(),
        nullable_bounded_number_schema(json!(-1), json!(1)),
    );
    grade_properties.insert(
        "highlights".into(),
        nullable_bounded_number_schema(json!(-1), json!(1)),
    );
    grade_properties.insert(
        "shadows".into(),
        nullable_bounded_number_schema(json!(-1), json!(1)),
    );
    grade_properties.insert(
        "blacks".into(),
        nullable_bounded_number_schema(json!(-1), json!(1)),
    );
    grade_properties.insert(
        "whites".into(),
        nullable_bounded_number_schema(json!(-1), json!(1)),
    );
    grade_properties.insert(
        "shadowsHue".into(),
        nullable_bounded_number_schema(json!(0), json!(360)),
    );
    grade_properties.insert(
        "shadowsAmount".into(),
        nullable_bounded_number_schema(json!(0), json!(1)),
    );
    grade_properties.insert(
        "shadowsLum".into(),
        nullable_bounded_number_schema(json!(-0.5), json!(0.5)),
    );
    grade_properties.insert(
        "midsHue".into(),
        nullable_bounded_number_schema(json!(0), json!(360)),
    );
    grade_properties.insert(
        "midsAmount".into(),
        nullable_bounded_number_schema(json!(0), json!(1)),
    );
    grade_properties.insert(
        "midsGamma".into(),
        nullable_bounded_number_schema(json!(0.5), json!(2)),
    );
    grade_properties.insert(
        "highsHue".into(),
        nullable_bounded_number_schema(json!(0), json!(360)),
    );
    grade_properties.insert(
        "highsAmount".into(),
        nullable_bounded_number_schema(json!(0), json!(1)),
    );
    grade_properties.insert(
        "highsGain".into(),
        nullable_bounded_number_schema(json!(0.5), json!(1.5)),
    );
    grade_properties.insert("masterCurve".into(), nullable_curve_points_schema());
    grade_properties.insert("redCurve".into(), nullable_curve_points_schema());
    grade_properties.insert("greenCurve".into(), nullable_curve_points_schema());
    grade_properties.insert("blueCurve".into(), nullable_curve_points_schema());
    grade_properties.insert("hueCurves".into(), nullable_hue_curves_schema());
    grade_properties.insert("lut".into(), nullable_lut_schema());
    // serde_json::Map can be insertion-ordered or key-ordered depending on the
    // unified feature graph. Keep this versioned schema stable in the editor's
    // processing order regardless of how Cargo composes the final binary.
    let grade_required = [
        "exposure",
        "contrast",
        "saturation",
        "temperature",
        "tint",
        "vibrance",
        "highlights",
        "shadows",
        "blacks",
        "whites",
        "shadowsHue",
        "shadowsAmount",
        "shadowsLum",
        "midsHue",
        "midsAmount",
        "midsGamma",
        "highsHue",
        "highsAmount",
        "highsGain",
        "masterCurve",
        "redCurve",
        "greenCurve",
        "blueCurve",
        "hueCurves",
        "lut",
    ];

    json!({
        "type": "object",
        "required": ["type", "itemIds", "reset", "grade"],
        "additionalProperties": false,
        "properties": {
            "type": {
                "type": "string",
                "enum": ["updateItemColorGrade"]
            },
            "itemIds": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string", "minLength": 1 }
            },
            "reset": { "type": "boolean" },
            "grade": {
                "type": "object",
                "required": grade_required,
                "additionalProperties": false,
                "properties": Value::Object(grade_properties)
            }
        }
    })
}

pub(crate) fn nullable_bounded_number_schema(minimum: Value, maximum: Value) -> Value {
    json!({
        "anyOf": [
            { "type": "number", "minimum": minimum, "maximum": maximum },
            { "type": "null" }
        ]
    })
}

pub(crate) fn nullable_curve_points_schema() -> Value {
    json!({
        "anyOf": [
            {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "array",
                    "minItems": 2,
                    "items": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1
                    }
                }
            },
            { "type": "null" }
        ]
    })
}

pub(crate) fn nullable_hue_curves_schema() -> Value {
    json!({
        "anyOf": [
            {
                "type": "object",
                "required": ["targets"],
                "additionalProperties": false,
                "properties": {
                    "targets": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["targetHue", "hueShift", "satScale", "lumShift"],
                            "additionalProperties": false,
                            "properties": {
                                "targetHue": { "type": "number", "minimum": 0, "maximum": 360 },
                                "hueShift": {
                                    "anyOf": [
                                        { "type": "number", "minimum": -30, "maximum": 30 },
                                        { "type": "null" }
                                    ]
                                },
                                "satScale": {
                                    "anyOf": [
                                        { "type": "number", "minimum": 0, "maximum": 2 },
                                        { "type": "null" }
                                    ]
                                },
                                "lumShift": {
                                    "anyOf": [
                                        { "type": "number", "minimum": -0.5, "maximum": 0.5 },
                                        { "type": "null" }
                                    ]
                                }
                            }
                        }
                    }
                }
            },
            { "type": "null" }
        ]
    })
}

pub(crate) fn nullable_lut_schema() -> Value {
    json!({
        "anyOf": [
            {
                "type": "object",
                "required": ["path", "strength"],
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "anyOf": [
                            { "type": "string", "minLength": 1 },
                            { "type": "null" }
                        ]
                    },
                    "strength": nullable_bounded_number_schema(json!(0), json!(1))
                }
            },
            { "type": "null" }
        ]
    })
}
