use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub min: f64,
    pub max: f64,
    pub default_value: f64,
    pub unit: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub params: Vec<EffectParamSpec>,
    pub linearizes: bool,
    pub resource_key: Option<&'static str>,
    pub color_effect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_schema: Option<Value>,
}

pub fn effect_catalog() -> Vec<EffectDescriptor> {
    vec![
        color_effect(
            "color.exposure",
            "Exposure",
            vec![param("ev", "Exposure", -3.0, 3.0, 0.0, "")],
            true,
            None,
        ),
        color_effect(
            "color.contrast",
            "Contrast",
            vec![param("amount", "Contrast", 0.5, 1.5, 1.0, "")],
            false,
            None,
        ),
        color_effect(
            "color.saturation",
            "Saturation",
            vec![param("amount", "Saturation", 0.0, 2.0, 1.0, "")],
            false,
            None,
        ),
        color_effect(
            "color.temperature",
            "Temperature & Tint",
            vec![
                param("temperature", "Temperature", 2000.0, 11000.0, 6500.0, "K"),
                param("tint", "Tint", -100.0, 100.0, 0.0, ""),
            ],
            false,
            None,
        ),
        color_effect(
            "color.highlightsShadows",
            "Highlights & Shadows",
            vec![
                param("highlights", "Highlights", -1.0, 1.0, 0.0, ""),
                param("shadows", "Shadows", -1.0, 1.0, 0.0, ""),
            ],
            false,
            None,
        ),
        color_effect(
            "color.blacksWhites",
            "Levels",
            vec![
                param("blacks", "Blacks", -1.0, 1.0, 0.0, ""),
                param("whites", "Whites", -1.0, 1.0, 0.0, ""),
            ],
            false,
            None,
        ),
        color_effect(
            "color.vibrance",
            "Vibrance",
            vec![param("amount", "Vibrance", -1.0, 1.0, 0.0, "")],
            false,
            None,
        ),
        color_effect(
            "color.wheels",
            "Color Wheels",
            vec![
                param("lift_x", "Lift", -1.0, 1.0, 0.0, ""),
                param("lift_y", "Lift", -1.0, 1.0, 0.0, ""),
                param("lift_m", "Lift", -0.5, 0.5, 0.0, ""),
                param("gamma_x", "Gamma", -1.0, 1.0, 0.0, ""),
                param("gamma_y", "Gamma", -1.0, 1.0, 0.0, ""),
                param("gamma_m", "Gamma", 0.5, 2.0, 1.0, ""),
                param("gain_x", "Gain", -1.0, 1.0, 0.0, ""),
                param("gain_y", "Gain", -1.0, 1.0, 0.0, ""),
                param("gain_m", "Gain", 0.5, 1.5, 1.0, ""),
            ],
            false,
            None,
        ),
        hue_curve_effect(),
        color_effect(
            "color.lut",
            "LUT",
            vec![param("intensity", "Intensity", 0.0, 1.0, 1.0, "")],
            false,
            Some("path"),
        ),
        curve_effect(),
        effect(
            "detail.clarity",
            "Clarity & Haze",
            "Detail",
            vec![
                param("clarity", "Clarity", -1.0, 1.0, 0.0, ""),
                param("dehaze", "Dehaze", -1.0, 1.0, 0.0, ""),
            ],
            None,
        ),
        effect(
            "key.chroma",
            "Chroma Key",
            "Key",
            vec![
                param("keyHue", "Key Hue", 0.0, 1.0, 0.333, ""),
                param("tolerance", "Tolerance", 0.0, 1.0, 0.0, ""),
                param("softness", "Softness", 0.0, 1.0, 0.5, ""),
                param("spill", "Spill", 0.0, 1.0, 0.5, ""),
            ],
            None,
        ),
        effect(
            "blur.gaussian",
            "Gaussian Blur",
            "Blur & Sharpen",
            vec![param("radius", "Radius", 0.0, 100.0, 8.0, "px")],
            None,
        ),
        effect(
            "blur.sharpen",
            "Sharpen",
            "Blur & Sharpen",
            vec![param("amount", "Sharpness", 0.0, 2.0, 0.4, "")],
            None,
        ),
        effect(
            "blur.noiseReduction",
            "Noise Reduction",
            "Blur & Sharpen",
            vec![param("amount", "Noise Reduction", 0.0, 1.0, 0.0, "")],
            None,
        ),
        effect(
            "audio.denoise",
            "Denoise Audio",
            "Audio",
            vec![param("amount", "Denoise", 0.0, 1.0, 0.6, "")],
            None,
        ),
        effect(
            "blur.motion",
            "Motion Blur",
            "Blur & Sharpen",
            vec![
                param("radius", "Motion Blur", 0.0, 100.0, 0.0, "px"),
                param("angle", "Angle", -180.0, 180.0, 0.0, "deg"),
            ],
            None,
        ),
        effect(
            "stylize.grain",
            "Film Grain",
            "Stylize",
            vec![
                param("amount", "Amount", 0.0, 1.0, 0.0, ""),
                param("size", "Size", 0.5, 4.0, 1.5, ""),
            ],
            None,
        ),
        effect(
            "stylize.vignette",
            "Vignette",
            "Stylize",
            vec![
                param("amount", "Amount", -1.0, 1.0, 0.0, ""),
                param("midpoint", "Midpoint", 0.0, 1.0, 0.5, ""),
                param("roundness", "Roundness", -1.0, 1.0, 0.0, ""),
                param("feather", "Feather", 0.0, 1.0, 0.5, ""),
            ],
            None,
        ),
        effect(
            "stylize.glow",
            "Glow",
            "Stylize",
            vec![
                param("intensity", "Glow", 0.0, 1.0, 0.0, ""),
                param("radius", "Radius", 0.0, 100.0, 20.0, "px"),
                param("threshold", "Threshold", 0.0, 1.0, 0.6, ""),
                param("warmth", "Warmth", 0.0, 1.0, 0.0, ""),
            ],
            None,
        ),
    ]
}

pub fn canonical_effect_order() -> Vec<&'static str> {
    vec![
        "color.exposure",
        "color.contrast",
        "color.highlightsShadows",
        "color.blacksWhites",
        "color.temperature",
        "color.vibrance",
        "color.saturation",
        "color.wheels",
        "color.curves",
        "color.hueCurves",
        "color.lut",
        "detail.clarity",
        "key.chroma",
        "blur.gaussian",
        "blur.sharpen",
        "blur.noiseReduction",
        "audio.denoise",
        "blur.motion",
        "stylize.grain",
        "stylize.vignette",
        "stylize.glow",
    ]
}

pub fn effect_descriptor(id: &str) -> Option<EffectDescriptor> {
    effect_catalog()
        .into_iter()
        .find(|effect| effect.id == id.trim())
}

pub fn canonical_effect_rank(id: &str) -> usize {
    canonical_effect_order()
        .iter()
        .position(|candidate| candidate == &id)
        .unwrap_or(usize::MAX)
}

pub fn effect_catalog_payload() -> Value {
    let effects = effect_catalog()
        .into_iter()
        .filter(|effect| {
            matches!(
                effect.id,
                "color.exposure"
                    | "color.contrast"
                    | "color.saturation"
                    | "color.temperature"
                    | "color.highlightsShadows"
                    | "color.blacksWhites"
                    | "color.vibrance"
                    | "color.wheels"
                    | "color.curves"
                    | "color.hueCurves"
                    | "color.lut"
                    | "detail.clarity"
                    | "key.chroma"
                    | "blur.gaussian"
                    | "blur.sharpen"
                    | "blur.noiseReduction"
                    | "blur.motion"
                    | "stylize.grain"
                    | "stylize.vignette"
                    | "stylize.glow"
            )
        })
        .collect::<Vec<_>>();
    let non_color_effects = effects
        .iter()
        .filter(|effect| !effect.color_effect)
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "source": "palmier-compatible",
        "effectCount": effects.len(),
        "canonicalOrder": canonical_effect_order()
            .into_iter()
            .filter(|id| effects.iter().any(|effect| effect.id == *id))
            .collect::<Vec<_>>(),
        "effects": effects,
        "nonColorEffects": non_color_effects,
    })
}

fn color_effect(
    id: &'static str,
    display_name: &'static str,
    params: Vec<EffectParamSpec>,
    linearizes: bool,
    resource_key: Option<&'static str>,
) -> EffectDescriptor {
    EffectDescriptor {
        id,
        display_name,
        category: "Color",
        params,
        linearizes,
        resource_key,
        color_effect: true,
        control_schema: None,
    }
}

fn effect(
    id: &'static str,
    display_name: &'static str,
    category: &'static str,
    params: Vec<EffectParamSpec>,
    resource_key: Option<&'static str>,
) -> EffectDescriptor {
    EffectDescriptor {
        id,
        display_name,
        category,
        params,
        linearizes: false,
        resource_key,
        color_effect: false,
        control_schema: None,
    }
}

fn curve_effect() -> EffectDescriptor {
    EffectDescriptor {
        id: "color.curves",
        display_name: "Curves",
        category: "Color",
        params: Vec::new(),
        linearizes: false,
        resource_key: None,
        color_effect: true,
        control_schema: Some(json!({
            "storage": "colorGrade",
            "keys": ["masterCurve", "redCurve", "greenCurve", "blueCurve"],
            "point": {
                "type": "tuple",
                "items": [
                    { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                    { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                ]
            },
            "minimumPoints": 2,
            "xOrder": "strictlyIncreasing",
            "interpolation": "piecewiseLinear",
            "outsideDomain": "clamp"
        })),
    }
}

fn hue_curve_effect() -> EffectDescriptor {
    EffectDescriptor {
        id: "color.hueCurves",
        display_name: "Hue Curves",
        category: "Color",
        params: Vec::new(),
        linearizes: false,
        resource_key: None,
        color_effect: true,
        control_schema: Some(json!({
            "storage": "colorGrade.hueCurves.targets",
            "entry": {
                "type": "object",
                "required": ["targetHue"],
                "additionalProperties": false,
                "properties": {
                    "targetHue": { "type": "number", "minimum": 0.0, "maximum": 360.0, "unit": "degrees" },
                    "hueShift": { "type": "number", "minimum": -30.0, "maximum": 30.0, "default": 0.0, "unit": "degrees" },
                    "satScale": { "type": "number", "minimum": 0.0, "maximum": 2.0, "default": 1.0 },
                    "lumShift": { "type": "number", "minimum": -0.5, "maximum": 0.5, "default": 0.0 }
                }
            },
            "minimumEntries": 1,
            "maximumEntries": 64,
            "domain": "normalizedCircularHue",
            "wrap": "360DegreesEquals0Degrees",
            "canonicalOrder": ["normalizedTargetHue", "hueShift", "satScale", "lumShift"],
            "supportRadiusDegrees": 60.0,
            "weighting": "smoothstepLocalized",
            "combination": "weightedDeltas"
        })),
    }
}

fn param(
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    default_value: f64,
    unit: &'static str,
) -> EffectParamSpec {
    EffectParamSpec {
        key,
        label,
        min,
        max,
        default_value,
        unit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_advertises_only_currently_executable_visual_effects() {
        let payload = effect_catalog_payload();
        let ids = payload["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .map(|effect| effect["id"].as_str().expect("id"))
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "color.exposure",
                "color.contrast",
                "color.saturation",
                "color.temperature",
                "color.highlightsShadows",
                "color.blacksWhites",
                "color.vibrance",
                "color.wheels",
                "color.hueCurves",
                "color.lut",
                "color.curves",
                "detail.clarity",
                "key.chroma",
                "blur.gaussian",
                "blur.sharpen",
                "blur.noiseReduction",
                "blur.motion",
                "stylize.grain",
                "stylize.vignette",
                "stylize.glow",
            ]
        );
        {
            let unsupported = "audio.denoise";
            assert!(!ids.contains(&unsupported), "{unsupported}");
        }
        let curves = payload["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .find(|effect| effect["id"] == "color.curves")
            .expect("curves");
        assert_eq!(curves["controlSchema"]["interpolation"], "piecewiseLinear");
        assert_eq!(curves["controlSchema"]["xOrder"], "strictlyIncreasing");
        let hue_curves = payload["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .find(|effect| effect["id"] == "color.hueCurves")
            .expect("hue curves");
        assert_eq!(
            hue_curves["controlSchema"]["domain"],
            "normalizedCircularHue"
        );
        assert_eq!(
            hue_curves["controlSchema"]["wrap"],
            "360DegreesEquals0Degrees"
        );
    }
}
