use crate::effects::{canonical_effect_rank, effect_descriptor};
use crate::frame_compositor::program::{KeyframeEasing, NumericCurve, NumericKeyframe};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

pub const SUPPORTED_CPU_EFFECTS: &[&str] = &[
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
    "blur.motion",
    "stylize.grain",
    "stylize.vignette",
    "stylize.glow",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedEffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_instance_id: Option<String>,
    pub effect_type: String,
    pub params: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub controls: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameter_keyframes: BTreeMap<String, NumericCurve>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedEffectStack {
    pub effects: Vec<PreparedEffect>,
}

#[derive(Debug, Error, PartialEq)]
pub enum EffectStackError {
    #[error("effect stack must be an array")]
    InvalidStack,
    #[error("effect stack entry {0} must be an object")]
    InvalidEntry(usize),
    #[error("effect stack entry {0} has an invalid effectType")]
    InvalidEffectType(usize),
    #[error("effect `{0}` is not supported by the deterministic CPU compositor")]
    UnsupportedEffect(String),
    #[error("effect `{effect_type}` parameter `{key}` is unknown")]
    UnknownParameter { effect_type: String, key: String },
    #[error(
        "effect `{effect_type}` parameter `{key}` must be a finite number between {min} and {max}"
    )]
    InvalidParameter {
        effect_type: String,
        key: String,
        min: f64,
        max: f64,
    },
    #[error("effect `{effect_type}` control `{key}` is invalid: {message}")]
    InvalidControl {
        effect_type: String,
        key: String,
        message: String,
    },
    #[error("effect parameter keyframes are invalid: {0}")]
    InvalidParameterKeyframes(String),
    #[error("effect frame dimensions or RGBA buffer length are invalid")]
    InvalidDimensions,
}

impl PreparedEffectStack {
    pub fn compose_child_then_wrapper(&self, wrapper: &Self) -> Self {
        let mut effects = self.effects.clone();
        effects.extend(wrapper.effects.clone());
        effects.sort_by_key(|effect| canonical_effect_rank(&effect.effect_type));
        Self { effects }
    }

    pub fn canonical_fingerprint(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("prepared effect stack is serializable");
        format!("{:x}", Sha256::digest(bytes))
    }

    pub fn from_value(value: Option<&Value>) -> Result<Self, EffectStackError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let entries = value.as_array().ok_or(EffectStackError::InvalidStack)?;
        let mut effects = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let object = entry
                .as_object()
                .ok_or(EffectStackError::InvalidEntry(index))?;
            if object.get("enabled").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            let effect_type = object
                .get("effectType")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or(EffectStackError::InvalidEffectType(index))?
                .trim();
            let effect_instance_id = object
                .get("effectInstanceId")
                .map(|value| {
                    value
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .ok_or_else(|| {
                            EffectStackError::InvalidParameterKeyframes(format!(
                                "effect stack entry {index} has an invalid effectInstanceId"
                            ))
                        })
                })
                .transpose()?;
            if !SUPPORTED_CPU_EFFECTS.contains(&effect_type) {
                return Err(EffectStackError::UnsupportedEffect(effect_type.to_string()));
            }
            let descriptor = effect_descriptor(effect_type)
                .ok_or_else(|| EffectStackError::UnsupportedEffect(effect_type.to_string()))?;
            let supplied = object.get("params").and_then(Value::as_object);
            if effect_type == "color.curves" {
                let supplied = supplied.ok_or_else(|| {
                    invalid_curve("masterCurve", "curve params must be an object")
                })?;
                let mut controls = BTreeMap::new();
                for (key, value) in supplied {
                    if !["masterCurve", "redCurve", "greenCurve", "blueCurve"]
                        .contains(&key.as_str())
                    {
                        return Err(EffectStackError::UnknownParameter {
                            effect_type: effect_type.to_string(),
                            key: key.clone(),
                        });
                    }
                    validate_curve_control(key, value)?;
                    controls.insert(key.clone(), value.clone());
                }
                if controls.is_empty() {
                    return Err(invalid_curve(
                        "masterCurve",
                        "at least one curve is required",
                    ));
                }
                effects.push(PreparedEffect {
                    effect_instance_id,
                    effect_type: effect_type.to_string(),
                    params: BTreeMap::new(),
                    controls,
                    parameter_keyframes: BTreeMap::new(),
                });
                continue;
            }
            if effect_type == "color.hueCurves" {
                let supplied =
                    supplied.ok_or_else(|| invalid_hue_curves("targets are required"))?;
                if let Some(key) = supplied.keys().find(|key| key.as_str() != "targets") {
                    return Err(EffectStackError::UnknownParameter {
                        effect_type: effect_type.to_string(),
                        key: key.clone(),
                    });
                }
                let targets = supplied
                    .get("targets")
                    .ok_or_else(|| invalid_hue_curves("targets are required"))?;
                effects.push(PreparedEffect {
                    effect_instance_id,
                    effect_type: effect_type.to_string(),
                    params: BTreeMap::new(),
                    controls: BTreeMap::from([(
                        "targets".to_string(),
                        canonical_hue_curve_targets(&json!({"targets": targets}))?,
                    )]),
                    parameter_keyframes: BTreeMap::new(),
                });
                continue;
            }
            if let Some(supplied) = supplied {
                for key in supplied.keys() {
                    if !descriptor.params.iter().any(|spec| spec.key == key)
                        && descriptor.resource_key != Some(key.as_str())
                    {
                        return Err(EffectStackError::UnknownParameter {
                            effect_type: effect_type.to_string(),
                            key: key.clone(),
                        });
                    }
                }
            }
            let mut params = BTreeMap::new();
            for spec in descriptor.params {
                let value = supplied
                    .and_then(|values| values.get(spec.key))
                    .map(|value| {
                        value
                            .as_f64()
                            .filter(|value| value.is_finite())
                            .ok_or_else(|| EffectStackError::InvalidParameter {
                                effect_type: effect_type.to_string(),
                                key: spec.key.to_string(),
                                min: spec.min,
                                max: spec.max,
                            })
                    })
                    .transpose()?
                    .unwrap_or(spec.default_value);
                if !(spec.min..=spec.max).contains(&value) {
                    return Err(EffectStackError::InvalidParameter {
                        effect_type: effect_type.to_string(),
                        key: spec.key.to_string(),
                        min: spec.min,
                        max: spec.max,
                    });
                }
                params.insert(spec.key.to_string(), value);
            }
            effects.push(PreparedEffect {
                effect_instance_id,
                effect_type: effect_type.to_string(),
                params,
                controls: BTreeMap::new(),
                parameter_keyframes: BTreeMap::new(),
            });
        }
        effects.sort_by_key(|effect| canonical_effect_rank(&effect.effect_type));
        Ok(Self { effects })
    }

    pub fn from_item_properties(
        properties: &BTreeMap<String, Value>,
    ) -> Result<Self, EffectStackError> {
        Self::from_item_properties_for_duration(properties, f64::INFINITY)
    }

    pub fn from_item_properties_for_duration(
        properties: &BTreeMap<String, Value>,
        duration_seconds: f64,
    ) -> Result<Self, EffectStackError> {
        let mut prepared = Self::from_value(properties.get("effects"))?;
        if let Some(grade) = properties.get("colorGrade") {
            prepared.effects.extend(effects_from_color_grade(grade)?);
            prepared
                .effects
                .sort_by_key(|effect| canonical_effect_rank(&effect.effect_type));
        }
        prepared.attach_parameter_keyframes(properties, duration_seconds)?;
        Ok(prepared)
    }

    pub fn sample(&self, local_seconds: f64) -> Self {
        let mut sampled = self.clone();
        for effect in &mut sampled.effects {
            for (parameter_key, curve) in &effect.parameter_keyframes {
                effect
                    .params
                    .insert(parameter_key.clone(), curve.sample(local_seconds));
            }
        }
        sampled
    }

    pub fn apply_rgba8_srgb_at_seconds(
        &self,
        source: &[u8],
        width: u32,
        height: u32,
        frame_index: u32,
        local_seconds: f64,
    ) -> Result<Vec<u8>, EffectStackError> {
        self.sample(local_seconds)
            .apply_rgba8_srgb(source, width, height, frame_index)
    }

    fn attach_parameter_keyframes(
        &mut self,
        properties: &BTreeMap<String, Value>,
        duration_seconds: f64,
    ) -> Result<(), EffectStackError> {
        let Some(value) = properties.get("effectParameterKeyframes") else {
            return Ok(());
        };
        let lanes = value.as_object().ok_or_else(|| {
            EffectStackError::InvalidParameterKeyframes(
                "effectParameterKeyframes must be an object".to_string(),
            )
        })?;
        let entries = properties
            .get("effects")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EffectStackError::InvalidParameterKeyframes(
                    "effectParameterKeyframes requires an effects array".to_string(),
                )
            })?;
        let mut metadata = BTreeMap::<String, (&str, bool)>::new();
        for (index, entry) in entries.iter().enumerate() {
            let Some(object) = entry.as_object() else {
                continue;
            };
            let Some(instance_id) = object
                .get("effectInstanceId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let effect_type = object
                .get("effectType")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if metadata
                .insert(
                    instance_id.to_string(),
                    (
                        effect_type,
                        object.get("enabled").and_then(Value::as_bool) != Some(false),
                    ),
                )
                .is_some()
            {
                return Err(EffectStackError::InvalidParameterKeyframes(format!(
                    "effectInstanceId `{instance_id}` is duplicated at entry {index}"
                )));
            }
        }
        for (instance_id, parameter_lanes) in lanes {
            let (effect_type, enabled) = metadata.get(instance_id).copied().ok_or_else(|| {
                EffectStackError::InvalidParameterKeyframes(format!(
                    "effectInstanceId `{instance_id}` does not identify an effect"
                ))
            })?;
            let descriptor = effect_descriptor(effect_type).ok_or_else(|| {
                EffectStackError::InvalidParameterKeyframes(format!(
                    "effectInstanceId `{instance_id}` has unknown effect type `{effect_type}`"
                ))
            })?;
            let parameter_lanes = parameter_lanes.as_object().ok_or_else(|| {
                EffectStackError::InvalidParameterKeyframes(format!(
                    "lanes for effectInstanceId `{instance_id}` must be an object"
                ))
            })?;
            for (parameter_key, lane) in parameter_lanes {
                let spec = descriptor
                    .params
                    .iter()
                    .find(|spec| spec.key == parameter_key)
                    .ok_or_else(|| {
                        EffectStackError::InvalidParameterKeyframes(format!(
                            "effect `{effect_type}` has no numeric parameter `{parameter_key}`"
                        ))
                    })?;
                let base = entries
                    .iter()
                    .filter_map(Value::as_object)
                    .find(|entry| {
                        entry.get("effectInstanceId").and_then(Value::as_str)
                            == Some(instance_id.as_str())
                    })
                    .and_then(|entry| entry.get("params"))
                    .and_then(Value::as_object)
                    .and_then(|params| params.get(parameter_key))
                    .and_then(Value::as_f64)
                    .unwrap_or(spec.default_value);
                let curve = parse_parameter_curve(
                    instance_id,
                    parameter_key,
                    lane,
                    base,
                    spec.min,
                    spec.max,
                    duration_seconds,
                )?;
                if enabled {
                    let effect = self
                        .effects
                        .iter_mut()
                        .find(|effect| effect.effect_instance_id.as_deref() == Some(instance_id))
                        .ok_or_else(|| {
                            EffectStackError::InvalidParameterKeyframes(format!(
                                "enabled effectInstanceId `{instance_id}` was not prepared"
                            ))
                        })?;
                    effect
                        .parameter_keyframes
                        .insert(parameter_key.clone(), curve);
                }
            }
        }
        Ok(())
    }

    pub fn apply_rgba8_srgb(
        &self,
        source: &[u8],
        width: u32,
        height: u32,
        frame_index: u32,
    ) -> Result<Vec<u8>, EffectStackError> {
        validate_frame(source, width, height)?;
        let mut frame = source.to_vec();
        for effect in &self.effects {
            frame = apply_effect(effect, &frame, width, height, frame_index);
        }
        Ok(frame)
    }

    pub fn before_lut(&self) -> Self {
        let lut_rank = canonical_effect_rank("color.lut");
        Self {
            effects: self
                .effects
                .iter()
                .filter(|effect| canonical_effect_rank(&effect.effect_type) < lut_rank)
                .cloned()
                .collect(),
        }
    }
}

fn parse_parameter_curve(
    instance_id: &str,
    parameter_key: &str,
    value: &Value,
    base: f64,
    minimum: f64,
    maximum: f64,
    duration_seconds: f64,
) -> Result<NumericCurve, EffectStackError> {
    let keyframes = value.as_array().ok_or_else(|| {
        EffectStackError::InvalidParameterKeyframes(format!(
            "lane `{instance_id}.{parameter_key}` must be an array"
        ))
    })?;
    let mut parsed = Vec::with_capacity(keyframes.len());
    let mut previous = None;
    for (index, keyframe) in keyframes.iter().enumerate() {
        let object = keyframe.as_object().ok_or_else(|| {
            EffectStackError::InvalidParameterKeyframes(format!(
                "keyframe {index} in `{instance_id}.{parameter_key}` must be an object"
            ))
        })?;
        let at_seconds = object
            .get("atSeconds")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                EffectStackError::InvalidParameterKeyframes(format!(
                    "keyframe {index} in `{instance_id}.{parameter_key}` is missing atSeconds"
                ))
            })?;
        let sample = object.get("value").and_then(Value::as_f64).ok_or_else(|| {
            EffectStackError::InvalidParameterKeyframes(format!(
                "keyframe {index} in `{instance_id}.{parameter_key}` is missing value"
            ))
        })?;
        if !at_seconds.is_finite()
            || at_seconds < 0.0
            || (duration_seconds.is_finite() && at_seconds > duration_seconds)
            || previous.is_some_and(|previous| at_seconds <= previous)
            || !sample.is_finite()
            || !(minimum..=maximum).contains(&sample)
        {
            return Err(EffectStackError::InvalidParameterKeyframes(format!(
                "keyframe {index} in `{instance_id}.{parameter_key}` is outside supported bounds"
            )));
        }
        let easing = match object
            .get("easing")
            .and_then(Value::as_str)
            .unwrap_or("linear")
        {
            "linear" => KeyframeEasing::Linear,
            "hold" => KeyframeEasing::Hold,
            "easeIn" => KeyframeEasing::EaseIn,
            "easeOut" => KeyframeEasing::EaseOut,
            "easeInOut" | "smooth" => KeyframeEasing::EaseInOut,
            easing => {
                return Err(EffectStackError::InvalidParameterKeyframes(format!(
                    "keyframe easing `{easing}` in `{instance_id}.{parameter_key}` is unsupported"
                )))
            }
        };
        previous = Some(at_seconds);
        parsed.push(NumericKeyframe {
            at_seconds,
            value: sample,
            easing,
        });
    }
    Ok(NumericCurve {
        base,
        keyframes: parsed,
    })
}

fn param(effect: &PreparedEffect, key: &str) -> f64 {
    effect.params.get(key).copied().unwrap_or_default()
}

fn effects_from_color_grade(value: &Value) -> Result<Vec<PreparedEffect>, EffectStackError> {
    let grade = value
        .as_object()
        .ok_or_else(|| EffectStackError::InvalidControl {
            effect_type: "colorGrade".to_string(),
            key: "colorGrade".to_string(),
            message: "must be an object".to_string(),
        })?;
    const RECOGNIZED: &[&str] = &[
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
    if let Some(key) = grade.keys().find(|key| !RECOGNIZED.contains(&key.as_str())) {
        return Err(EffectStackError::UnknownParameter {
            effect_type: "colorGrade".to_string(),
            key: key.clone(),
        });
    }
    for key in [
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
    ] {
        if grade
            .get(key)
            .is_some_and(|value| value.as_f64().is_none_or(|value| !value.is_finite()))
        {
            return Err(EffectStackError::InvalidControl {
                effect_type: "colorGrade".to_string(),
                key: key.to_string(),
                message: "must be a finite number".to_string(),
            });
        }
    }
    let number = |key: &str| grade.get(key).and_then(Value::as_f64);
    let mut effects = Vec::new();
    if let Some(value) = number("exposure") {
        effects.push(prepared_numeric("color.exposure", &[("ev", value)])?);
    }
    if let Some(value) = number("contrast") {
        effects.push(prepared_numeric("color.contrast", &[("amount", value)])?);
    }
    if number("highlights").is_some() || number("shadows").is_some() {
        effects.push(prepared_numeric(
            "color.highlightsShadows",
            &[
                ("highlights", number("highlights").unwrap_or(0.0)),
                ("shadows", number("shadows").unwrap_or(0.0)),
            ],
        )?);
    }
    if number("blacks").is_some() || number("whites").is_some() {
        effects.push(prepared_numeric(
            "color.blacksWhites",
            &[
                ("blacks", number("blacks").unwrap_or(0.0)),
                ("whites", number("whites").unwrap_or(0.0)),
            ],
        )?);
    }
    if number("temperature").is_some() || number("tint").is_some() {
        effects.push(prepared_numeric(
            "color.temperature",
            &[
                ("temperature", number("temperature").unwrap_or(6500.0)),
                ("tint", number("tint").unwrap_or(0.0)),
            ],
        )?);
    }
    if let Some(value) = number("vibrance") {
        effects.push(prepared_numeric("color.vibrance", &[("amount", value)])?);
    }
    if let Some(value) = number("saturation") {
        effects.push(prepared_numeric("color.saturation", &[("amount", value)])?);
    }
    if [
        "shadowsHue",
        "shadowsAmount",
        "shadowsLum",
        "midsHue",
        "midsAmount",
        "midsGamma",
        "highsHue",
        "highsAmount",
        "highsGain",
    ]
    .iter()
    .any(|key| grade.contains_key(*key))
    {
        let wheel = |hue: &str, amount: &str| {
            let radians = number(hue).unwrap_or(0.0).to_radians();
            let magnitude = number(amount).unwrap_or(0.0);
            (radians.cos() * magnitude, radians.sin() * magnitude)
        };
        let lift = wheel("shadowsHue", "shadowsAmount");
        let gamma = wheel("midsHue", "midsAmount");
        let gain = wheel("highsHue", "highsAmount");
        effects.push(prepared_numeric(
            "color.wheels",
            &[
                ("lift_x", lift.0),
                ("lift_y", lift.1),
                ("lift_m", number("shadowsLum").unwrap_or(0.0)),
                ("gamma_x", gamma.0),
                ("gamma_y", gamma.1),
                ("gamma_m", number("midsGamma").unwrap_or(1.0)),
                ("gain_x", gain.0),
                ("gain_y", gain.1),
                ("gain_m", number("highsGain").unwrap_or(1.0)),
            ],
        )?);
    }
    let mut controls = BTreeMap::new();
    for key in ["masterCurve", "redCurve", "greenCurve", "blueCurve"] {
        if let Some(value) = grade.get(key) {
            validate_curve_control(key, value)?;
            controls.insert(key.to_string(), value.clone());
        }
    }
    if !controls.is_empty() {
        effects.push(PreparedEffect {
            effect_instance_id: None,
            effect_type: "color.curves".to_string(),
            params: BTreeMap::new(),
            controls,
            parameter_keyframes: BTreeMap::new(),
        });
    }
    if let Some(value) = grade.get("hueCurves") {
        effects.push(PreparedEffect {
            effect_instance_id: None,
            effect_type: "color.hueCurves".to_string(),
            params: BTreeMap::new(),
            controls: BTreeMap::from([(
                "targets".to_string(),
                canonical_hue_curve_targets(value)?,
            )]),
            parameter_keyframes: BTreeMap::new(),
        });
    }
    Ok(effects)
}

fn canonical_hue_curve_targets(value: &Value) -> Result<Value, EffectStackError> {
    let targets = value
        .get("targets")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_hue_curves("targets must be an array"))?;
    if targets.is_empty() {
        return Err(invalid_hue_curves(
            "targets must include at least one entry",
        ));
    }
    if targets.len() > 64 {
        return Err(invalid_hue_curves(
            "targets must include at most 64 entries",
        ));
    }
    let mut canonical = Vec::with_capacity(targets.len());
    for target in targets {
        let target = target
            .as_object()
            .ok_or_else(|| invalid_hue_curves("each target must be an object"))?;
        if let Some(key) = target
            .keys()
            .find(|key| !["targetHue", "hueShift", "satScale", "lumShift"].contains(&key.as_str()))
        {
            return Err(invalid_hue_curves(&format!(
                "unknown target control `{key}`"
            )));
        }
        let bounded = |key: &str, default: f64, minimum: f64, maximum: f64| {
            let value = match target.get(key) {
                None => default,
                Some(value) => value.as_f64().ok_or(())?,
            };
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err(());
            }
            Ok(value)
        };
        let target_hue = bounded("targetHue", 0.0, 0.0, 360.0)
            .map_err(|_| invalid_hue_curves("targetHue must be finite from 0 through 360"))?
            .rem_euclid(360.0);
        let hue_shift = bounded("hueShift", 0.0, -30.0, 30.0)
            .map_err(|_| invalid_hue_curves("hueShift must be finite from -30 through 30"))?;
        let sat_scale = bounded("satScale", 1.0, 0.0, 2.0)
            .map_err(|_| invalid_hue_curves("satScale must be finite from 0 through 2"))?;
        let lum_shift = bounded("lumShift", 0.0, -0.5, 0.5)
            .map_err(|_| invalid_hue_curves("lumShift must be finite from -0.5 through 0.5"))?;
        canonical.push(json!({
            "targetHue": target_hue,
            "hueShift": hue_shift,
            "satScale": sat_scale,
            "lumShift": lum_shift,
        }));
    }
    canonical.sort_by(|left, right| {
        ["targetHue", "hueShift", "satScale", "lumShift"]
            .into_iter()
            .find_map(|key| {
                let ordering = left[key]
                    .as_f64()
                    .expect("canonical hue value")
                    .total_cmp(&right[key].as_f64().expect("canonical hue value"));
                (ordering != std::cmp::Ordering::Equal).then_some(ordering)
            })
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(Value::Array(canonical))
}

fn invalid_hue_curves(message: &str) -> EffectStackError {
    EffectStackError::InvalidControl {
        effect_type: "color.hueCurves".to_string(),
        key: "targets".to_string(),
        message: message.to_string(),
    }
}

fn prepared_numeric(
    id: &str,
    overrides: &[(&str, f64)],
) -> Result<PreparedEffect, EffectStackError> {
    let descriptor =
        effect_descriptor(id).ok_or_else(|| EffectStackError::UnsupportedEffect(id.to_string()))?;
    let specs = descriptor.params;
    let mut params = specs
        .iter()
        .map(|spec| (spec.key.to_string(), spec.default_value))
        .collect::<BTreeMap<_, _>>();
    for (key, value) in overrides {
        let spec = specs.iter().find(|spec| spec.key == *key).ok_or_else(|| {
            EffectStackError::UnknownParameter {
                effect_type: id.to_string(),
                key: (*key).to_string(),
            }
        })?;
        if !value.is_finite() || !(spec.min..=spec.max).contains(value) {
            return Err(EffectStackError::InvalidParameter {
                effect_type: id.to_string(),
                key: (*key).to_string(),
                min: spec.min,
                max: spec.max,
            });
        }
        params.insert((*key).to_string(), *value);
    }
    Ok(PreparedEffect {
        effect_instance_id: None,
        effect_type: id.to_string(),
        params,
        controls: BTreeMap::new(),
        parameter_keyframes: BTreeMap::new(),
    })
}

fn validate_curve_control(key: &str, value: &Value) -> Result<(), EffectStackError> {
    let points = value
        .as_array()
        .ok_or_else(|| invalid_curve(key, "must be an array of [x, y] points"))?;
    if points.len() < 2 {
        return Err(invalid_curve(key, "must include at least two points"));
    }
    let mut previous = None;
    for point in points {
        let pair = point
            .as_array()
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| invalid_curve(key, "points must be [x, y] pairs"))?;
        let x = pair[0]
            .as_f64()
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .ok_or_else(|| invalid_curve(key, "x must be finite from 0 through 1"))?;
        pair[1]
            .as_f64()
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .ok_or_else(|| invalid_curve(key, "y must be finite from 0 through 1"))?;
        if previous.is_some_and(|previous| x <= previous) {
            return Err(invalid_curve(key, "x values must be strictly increasing"));
        }
        previous = Some(x);
    }
    Ok(())
}

fn invalid_curve(key: &str, message: &str) -> EffectStackError {
    EffectStackError::InvalidControl {
        effect_type: "color.curves".to_string(),
        key: key.to_string(),
        message: message.to_string(),
    }
}

fn apply_effect(
    effect: &PreparedEffect,
    source: &[u8],
    width: u32,
    height: u32,
    frame: u32,
) -> Vec<u8> {
    match effect.effect_type.as_str() {
        "color.exposure" => exposure(source, param(effect, "ev")),
        "color.contrast" => contrast(source, param(effect, "amount")),
        "color.highlightsShadows" => highlights_shadows(
            source,
            param(effect, "highlights"),
            param(effect, "shadows"),
        ),
        "color.blacksWhites" => {
            blacks_whites(source, param(effect, "blacks"), param(effect, "whites"))
        }
        "color.temperature" => {
            temperature_tint(source, param(effect, "temperature"), param(effect, "tint"))
        }
        "color.vibrance" => vibrance(source, param(effect, "amount")),
        "color.saturation" => saturation(source, param(effect, "amount")),
        "color.wheels" => color_wheels(effect, source),
        "color.curves" => color_curves(effect, source),
        "color.hueCurves" => hue_curves(effect, source),
        "detail.clarity" => clarity(
            source,
            width,
            height,
            param(effect, "clarity"),
            param(effect, "dehaze"),
        ),
        "key.chroma" => chroma_key(
            source,
            param(effect, "keyHue"),
            param(effect, "tolerance"),
            param(effect, "softness"),
            param(effect, "spill"),
        ),
        "blur.gaussian" => box_blur(
            source,
            width,
            height,
            param(effect, "radius").round() as u32,
        ),
        "blur.sharpen" => sharpen(source, width, height, param(effect, "amount")),
        "blur.noiseReduction" => mix_frames(
            source,
            &box_blur(source, width, height, 1),
            param(effect, "amount"),
        ),
        "blur.motion" => motion_blur(
            source,
            width,
            height,
            param(effect, "radius").round() as u32,
            param(effect, "angle"),
        ),
        "stylize.grain" => grain(
            source,
            width,
            frame,
            param(effect, "amount"),
            param(effect, "size"),
        ),
        "stylize.vignette" => vignette(
            source,
            width,
            height,
            param(effect, "amount"),
            param(effect, "midpoint"),
            param(effect, "roundness"),
            param(effect, "feather"),
        ),
        "stylize.glow" => glow(
            source,
            width,
            height,
            param(effect, "intensity"),
            param(effect, "radius").round() as u32,
            param(effect, "threshold"),
            param(effect, "warmth"),
        ),
        _ => source.to_vec(),
    }
}

fn validate_frame(source: &[u8], width: u32, height: u32) -> Result<(), EffectStackError> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .and_then(|n| n.checked_mul(4));
    if width == 0 || height == 0 || expected != Some(source.len()) {
        return Err(EffectStackError::InvalidDimensions);
    }
    Ok(())
}

fn map_rgb(source: &[u8], mut map: impl FnMut([f64; 3]) -> [f64; 3]) -> Vec<u8> {
    let mut output = source.to_vec();
    for pixel in output.chunks_exact_mut(4) {
        let mapped = map([
            f64::from(pixel[0]) / 255.0,
            f64::from(pixel[1]) / 255.0,
            f64::from(pixel[2]) / 255.0,
        ]);
        for channel in 0..3 {
            pixel[channel] = clamp_u8(mapped[channel] * 255.0);
        }
    }
    output
}

fn exposure(source: &[u8], ev: f64) -> Vec<u8> {
    let multiplier = 2_f64.powf(ev);
    map_rgb(source, |rgb| {
        rgb.map(|channel| linear_to_srgb_f64(srgb_to_linear_f64(channel) * multiplier))
    })
}

fn contrast(source: &[u8], amount: f64) -> Vec<u8> {
    map_rgb(source, |rgb| {
        rgb.map(|channel| 0.5 + (channel - 0.5) * amount)
    })
}

fn saturation(source: &[u8], amount: f64) -> Vec<u8> {
    map_rgb(source, |rgb| {
        let luma = rgb_luma(rgb);
        rgb.map(|channel| luma + (channel - luma) * amount)
    })
}

fn vibrance(source: &[u8], amount: f64) -> Vec<u8> {
    map_rgb(source, |rgb| {
        let maximum = rgb[0].max(rgb[1]).max(rgb[2]);
        let minimum = rgb[0].min(rgb[1]).min(rgb[2]);
        let chroma = maximum - minimum;
        let adaptive = 1.0 + amount * (1.0 - chroma);
        let luma = rgb_luma(rgb);
        rgb.map(|channel| luma + (channel - luma) * adaptive)
    })
}

fn temperature_tint(source: &[u8], temperature: f64, tint: f64) -> Vec<u8> {
    let warmth = ((temperature - 6500.0) / 4500.0).clamp(-1.0, 1.0);
    let tint = (tint / 100.0).clamp(-1.0, 1.0);
    map_rgb(source, |rgb| {
        [
            rgb[0] * (1.0 + warmth * 0.18 + tint * 0.08),
            rgb[1] * (1.0 - tint * 0.12),
            rgb[2] * (1.0 - warmth * 0.18 + tint * 0.08),
        ]
    })
}

fn highlights_shadows(source: &[u8], highlights: f64, shadows: f64) -> Vec<u8> {
    map_rgb(source, |rgb| {
        let luma = rgb_luma(rgb);
        let adjustment = shadows * (1.0 - luma).powi(2) * 0.5 + highlights * luma.powi(2) * 0.5;
        rgb.map(|channel| channel + adjustment)
    })
}

fn blacks_whites(source: &[u8], blacks: f64, whites: f64) -> Vec<u8> {
    map_rgb(source, |rgb| {
        let luma = rgb_luma(rgb);
        let adjustment = blacks * (1.0 - luma).powi(3) * 0.35 + whites * luma.powi(3) * 0.35;
        rgb.map(|channel| channel + adjustment)
    })
}

fn color_wheels(effect: &PreparedEffect, source: &[u8]) -> Vec<u8> {
    let lift = wheel_offset(
        param(effect, "lift_x"),
        param(effect, "lift_y"),
        param(effect, "lift_m"),
    );
    let gamma = wheel_offset(param(effect, "gamma_x"), param(effect, "gamma_y"), 0.0);
    let gain = wheel_offset(param(effect, "gain_x"), param(effect, "gain_y"), 0.0);
    let gamma_m = param(effect, "gamma_m").max(0.01);
    let gain_m = param(effect, "gain_m");
    map_rgb(source, |rgb| {
        let luma = rgb_luma(rgb);
        let shadow = (1.0 - luma).powi(2);
        let mid = (1.0 - (2.0 * luma - 1.0).abs()).max(0.0);
        let high = luma.powi(2);
        std::array::from_fn(|channel| {
            let lifted = rgb[channel] + lift[channel] * shadow;
            let gamma_adjusted = lifted.max(0.0).powf(1.0 / gamma_m) + gamma[channel] * mid;
            gamma_adjusted * gain_m + gain[channel] * high
        })
    })
}

fn wheel_offset(x: f64, y: f64, magnitude_offset: f64) -> [f64; 3] {
    let magnitude = x.hypot(y);
    if magnitude <= f64::EPSILON {
        return [magnitude_offset; 3];
    }
    let hue = y.atan2(x).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
    let rgb = hsv_to_rgb(hue, 1.0, 1.0);
    rgb.map(|channel| (channel - 1.0 / 3.0) * magnitude * 0.3 + magnitude_offset)
}

fn color_curves(effect: &PreparedEffect, source: &[u8]) -> Vec<u8> {
    map_rgb(source, |mut rgb| {
        if let Some(points) = effect.controls.get("masterCurve") {
            rgb = rgb.map(|value| sample_curve(points, value));
        }
        for (channel, key) in ["redCurve", "greenCurve", "blueCurve"]
            .into_iter()
            .enumerate()
        {
            if let Some(points) = effect.controls.get(key) {
                rgb[channel] = sample_curve(points, rgb[channel]);
            }
        }
        rgb
    })
}

fn hue_curves(effect: &PreparedEffect, source: &[u8]) -> Vec<u8> {
    let targets = effect
        .controls
        .get("targets")
        .and_then(Value::as_array)
        .expect("validated hue curve targets");
    let mut output = source.to_vec();
    for pixel in output.chunks_exact_mut(4) {
        let (hue, saturation, value) = rgb_to_hsv(pixel[0], pixel[1], pixel[2]);
        let mut hue_delta = 0.0;
        let mut saturation_delta = 0.0;
        let mut value_delta = 0.0;
        for target in targets {
            let target_hue = target["targetHue"].as_f64().expect("canonical target hue") / 360.0;
            let distance = circular_distance(hue, target_hue);
            let normalized = (1.0 - distance / (1.0 / 6.0)).clamp(0.0, 1.0);
            let weight = normalized * normalized * (3.0 - 2.0 * normalized);
            hue_delta += target["hueShift"].as_f64().expect("canonical hue shift") / 360.0 * weight;
            saturation_delta +=
                (target["satScale"].as_f64().expect("canonical saturation") - 1.0) * weight;
            value_delta += target["lumShift"].as_f64().expect("canonical luminance") * weight;
        }
        let mapped = hsv_to_rgb(
            (hue + hue_delta).rem_euclid(1.0),
            (saturation * (1.0 + saturation_delta)).clamp(0.0, 1.0),
            (value + value_delta).clamp(0.0, 1.0),
        );
        for channel in 0..3 {
            pixel[channel] = clamp_u8(mapped[channel] * 255.0);
        }
    }
    output
}

fn circular_distance(left: f64, right: f64) -> f64 {
    let distance = (left - right).abs().rem_euclid(1.0);
    distance.min(1.0 - distance)
}

fn sample_curve(points: &Value, value: f64) -> f64 {
    let points = points.as_array().expect("validated curve points");
    let point = |index: usize| {
        let pair = points[index].as_array().expect("validated curve pair");
        (
            pair[0].as_f64().expect("validated curve x"),
            pair[1].as_f64().expect("validated curve y"),
        )
    };
    let first = point(0);
    if value <= first.0 {
        return first.1;
    }
    for index in 1..points.len() {
        let right = point(index);
        if value <= right.0 {
            let left = point(index - 1);
            let progress = (value - left.0) / (right.0 - left.0);
            return left.1 + (right.1 - left.1) * progress;
        }
    }
    point(points.len() - 1).1
}

fn rgb_luma(rgb: [f64; 3]) -> f64 {
    rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722
}
fn srgb_to_linear_f64(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn linear_to_srgb_f64(value: f64) -> f64 {
    let value = value.max(0.0);
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn box_blur(source: &[u8], width: u32, height: u32, radius: u32) -> Vec<u8> {
    if radius == 0 {
        return source.to_vec();
    }
    let radius = radius.min(100);
    let mut horizontal = vec![0_u8; source.len()];
    for y in 0..height {
        let mut sum = [0_u64; 4];
        let initial_end = radius.min(width - 1);
        for x in 0..=initial_end {
            add_pixel(&mut sum, source, width, x, y);
        }
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = x.saturating_add(radius).min(width - 1);
            write_average(
                &mut horizontal,
                width,
                x,
                y,
                sum,
                u64::from(end - start + 1),
            );
            if x >= radius {
                subtract_pixel(&mut sum, source, width, x - radius, y);
            }
            if let Some(add_x) = x.checked_add(radius + 1).filter(|next| *next < width) {
                add_pixel(&mut sum, source, width, add_x, y);
            }
        }
    }
    let mut output = vec![0_u8; source.len()];
    for x in 0..width {
        let mut sum = [0_u64; 4];
        let initial_end = radius.min(height - 1);
        for y in 0..=initial_end {
            add_pixel(&mut sum, &horizontal, width, x, y);
        }
        for y in 0..height {
            let start = y.saturating_sub(radius);
            let end = y.saturating_add(radius).min(height - 1);
            write_average(&mut output, width, x, y, sum, u64::from(end - start + 1));
            if y >= radius {
                subtract_pixel(&mut sum, &horizontal, width, x, y - radius);
            }
            if let Some(add_y) = y.checked_add(radius + 1).filter(|next| *next < height) {
                add_pixel(&mut sum, &horizontal, width, x, add_y);
            }
        }
    }
    output
}

fn add_pixel(sum: &mut [u64; 4], source: &[u8], width: u32, x: u32, y: u32) {
    let offset = ((y * width + x) * 4) as usize;
    for channel in 0..4 {
        sum[channel] += u64::from(source[offset + channel]);
    }
}

fn subtract_pixel(sum: &mut [u64; 4], source: &[u8], width: u32, x: u32, y: u32) {
    let offset = ((y * width + x) * 4) as usize;
    for channel in 0..4 {
        sum[channel] -= u64::from(source[offset + channel]);
    }
}

fn write_average(output: &mut [u8], width: u32, x: u32, y: u32, sum: [u64; 4], count: u64) {
    let offset = ((y * width + x) * 4) as usize;
    for channel in 0..4 {
        output[offset + channel] = ((sum[channel] + count / 2) / count) as u8;
    }
}

fn mix_frames(a: &[u8], b: &[u8], amount: f64) -> Vec<u8> {
    a.iter()
        .zip(b)
        .enumerate()
        .map(|(i, (&x, &y))| if i % 4 == 3 { x } else { lerp_u8(x, y, amount) })
        .collect()
}

fn sharpen(source: &[u8], width: u32, height: u32, amount: f64) -> Vec<u8> {
    let blurred = box_blur(source, width, height, 1);
    source
        .iter()
        .zip(blurred)
        .enumerate()
        .map(|(i, (&x, y))| {
            if i % 4 == 3 {
                x
            } else {
                clamp_u8(f64::from(x) + amount * (f64::from(x) - f64::from(y)))
            }
        })
        .collect()
}

fn clarity(source: &[u8], width: u32, height: u32, clarity: f64, dehaze: f64) -> Vec<u8> {
    let blur = box_blur(source, width, height, 1);
    source
        .iter()
        .zip(blur)
        .enumerate()
        .map(|(i, (&x, y))| {
            if i % 4 == 3 {
                x
            } else {
                let detailed = f64::from(x) + clarity * (f64::from(x) - f64::from(y));
                clamp_u8(128.0 + (detailed - 128.0) * (1.0 + dehaze * 0.75))
            }
        })
        .collect()
}

fn chroma_key(source: &[u8], key_hue: f64, tolerance: f64, softness: f64, spill: f64) -> Vec<u8> {
    let mut out = source.to_vec();
    for pixel in out.chunks_exact_mut(4) {
        let (h, s, _) = rgb_to_hsv(pixel[0], pixel[1], pixel[2]);
        let distance = ((h - key_hue).abs()).min(1.0 - (h - key_hue).abs());
        let edge = (tolerance + softness * 0.25).max(1e-6);
        let keyed = ((edge - distance) / edge).clamp(0.0, 1.0) * s;
        pixel[3] = clamp_u8(f64::from(pixel[3]) * (1.0 - keyed));
        pixel[1] = clamp_u8(f64::from(pixel[1]) * (1.0 - keyed * spill * 0.5));
    }
    out
}

fn motion_blur(source: &[u8], width: u32, height: u32, radius: u32, angle: f64) -> Vec<u8> {
    if radius == 0 {
        return source.to_vec();
    }
    let r = radius.min(100) as i32;
    let sample_step = ((r * 2 + 31) / 32).max(1);
    let rad = angle.to_radians();
    let dx = rad.cos();
    let dy = rad.sin();
    let mut out = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0_u64; 4];
            let mut count = 0;
            for step in (-r..=r).step_by(sample_step as usize) {
                let sx = (f64::from(x) + f64::from(step) * dx)
                    .round()
                    .clamp(0.0, f64::from(width - 1)) as u32;
                let sy = (f64::from(y) + f64::from(step) * dy)
                    .round()
                    .clamp(0.0, f64::from(height - 1)) as u32;
                let o = ((sy * width + sx) * 4) as usize;
                for c in 0..4 {
                    sum[c] += u64::from(source[o + c]);
                }
                count += 1;
            }
            let o = ((y * width + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = ((sum[c] + count / 2) / count) as u8;
            }
        }
    }
    out
}

fn grain(source: &[u8], width: u32, frame: u32, amount: f64, size: f64) -> Vec<u8> {
    let mut out = source.to_vec();
    for (index, pixel) in out.chunks_exact_mut(4).enumerate() {
        let x = index as u32 % width;
        let y = index as u32 / width;
        let cell = (size.max(0.5) * 2.0).round() as u32;
        let mut n = (x / cell).wrapping_mul(0x9e3779b9)
            ^ (y / cell).wrapping_mul(0x85ebca6b)
            ^ frame.wrapping_mul(0xc2b2ae35);
        n ^= n >> 16;
        n = n.wrapping_mul(0x7feb352d);
        n ^= n >> 15;
        let noise = (f64::from(n & 0xffff) / 65535.0 - 0.5) * 2.0 * amount * 64.0;
        for channel in &mut pixel[..3] {
            *channel = clamp_u8(f64::from(*channel) + noise);
        }
    }
    out
}

fn vignette(
    source: &[u8],
    width: u32,
    height: u32,
    amount: f64,
    midpoint: f64,
    roundness: f64,
    feather: f64,
) -> Vec<u8> {
    let mut out = source.to_vec();
    for y in 0..height {
        for x in 0..width {
            let nx = (f64::from(x) + 0.5) / f64::from(width) * 2.0 - 1.0;
            let ny = (f64::from(y) + 0.5) / f64::from(height) * 2.0 - 1.0;
            let aspect = 1.0 + roundness.abs();
            let d = ((nx * aspect).powi(2) + (ny / aspect).powi(2)).sqrt() / 2_f64.sqrt();
            let edge =
                ((d - midpoint) / (feather.max(0.01) * (1.0 - midpoint).max(0.01))).clamp(0.0, 1.0);
            let factor = 1.0 + amount * edge;
            let o = ((y * width + x) * 4) as usize;
            for c in 0..3 {
                out[o + c] = clamp_u8(f64::from(out[o + c]) * factor);
            }
        }
    }
    out
}

fn glow(
    source: &[u8],
    width: u32,
    height: u32,
    intensity: f64,
    radius: u32,
    threshold: f64,
    warmth: f64,
) -> Vec<u8> {
    if intensity == 0.0 {
        return source.to_vec();
    }
    let mut bright = vec![0; source.len()];
    for (src, dst) in source.chunks_exact(4).zip(bright.chunks_exact_mut(4)) {
        let l =
            (0.2126 * f64::from(src[0]) + 0.7152 * f64::from(src[1]) + 0.0722 * f64::from(src[2]))
                / 255.0;
        let gate = ((l - threshold) / (1.0 - threshold).max(1e-6)).clamp(0.0, 1.0);
        dst[0] = clamp_u8(f64::from(src[0]) * gate * (1.0 + warmth * 0.25));
        dst[1] = clamp_u8(f64::from(src[1]) * gate);
        dst[2] = clamp_u8(f64::from(src[2]) * gate * (1.0 - warmth * 0.25));
        dst[3] = src[3];
    }
    let blur = box_blur(&bright, width, height, radius);
    source
        .iter()
        .zip(blur)
        .enumerate()
        .map(|(i, (&x, y))| {
            if i % 4 == 3 {
                x
            } else {
                clamp_u8(f64::from(x) + f64::from(y) * intensity)
            }
        })
        .collect()
}

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = f64::from(r) / 255.0;
    let g = f64::from(g) / 255.0;
    let b = f64::from(b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= f64::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / d / 6.0).rem_euclid(1.0)
    } else if max == g {
        (b - r) / d / 6.0 + 1.0 / 3.0
    } else {
        (r - g) / d / 6.0 + 2.0 / 3.0
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    let sector = (h.rem_euclid(1.0) * 6.0).floor() as i32;
    let fraction = h.rem_euclid(1.0) * 6.0 - f64::from(sector);
    let p = v * (1.0 - s);
    let q = v * (1.0 - fraction * s);
    let t = v * (1.0 - (1.0 - fraction) * s);
    match sector.rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}
fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    clamp_u8(f64::from(a) + (f64::from(b) - f64::from(a)) * t.clamp(0.0, 1.0))
}
fn clamp_u8(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn stack(effect_type: &str, params: Value) -> PreparedEffectStack {
        PreparedEffectStack::from_value(Some(
            &json!([{"effectType":effect_type,"enabled":true,"params":params}]),
        ))
        .unwrap()
    }
    fn fixture() -> Vec<u8> {
        vec![
            20, 40, 60, 255, 80, 100, 120, 255, 140, 160, 180, 192, 220, 240, 250, 128,
        ]
    }

    #[test]
    fn each_supported_effect_is_deterministic_and_preserves_rgba_shape() {
        let cases = [
            ("color.exposure", json!({"ev":1.0})),
            ("color.contrast", json!({"amount":1.5})),
            (
                "color.highlightsShadows",
                json!({"highlights":0.5,"shadows":0.5}),
            ),
            ("color.blacksWhites", json!({"blacks":0.5,"whites":0.5})),
            (
                "color.temperature",
                json!({"temperature":9000.0,"tint":25.0}),
            ),
            ("color.vibrance", json!({"amount":1.0})),
            ("color.saturation", json!({"amount":0.0})),
            (
                "color.wheels",
                json!({"lift_x":0.5,"gamma_y":0.5,"gain_m":1.25}),
            ),
            ("detail.clarity", json!({"clarity":1.0,"dehaze":0.5})),
            (
                "key.chroma",
                json!({"keyHue":0.333,"tolerance":1.0,"softness":0.0,"spill":0.5}),
            ),
            ("blur.gaussian", json!({"radius":1.0})),
            ("blur.sharpen", json!({"amount":2.0})),
            ("blur.noiseReduction", json!({"amount":1.0})),
            ("blur.motion", json!({"radius":1.0,"angle":0.0})),
            ("stylize.grain", json!({"amount":1.0,"size":1.0})),
            (
                "stylize.vignette",
                json!({"amount":-1.0,"midpoint":0.0,"feather":0.5}),
            ),
            (
                "stylize.glow",
                json!({"intensity":1.0,"radius":1.0,"threshold":0.0,"warmth":0.5}),
            ),
        ];
        // Curves and hue curves have dedicated typed-control coverage below;
        // LUT resources are consumed by the precompose stage.
        assert_eq!(cases.len() + 3, SUPPORTED_CPU_EFFECTS.len());
        for (id, params) in cases {
            let first = stack(id, params.clone())
                .apply_rgba8_srgb(&fixture(), 2, 2, 7)
                .unwrap();
            let second = stack(id, params)
                .apply_rgba8_srgb(&fixture(), 2, 2, 7)
                .unwrap();
            assert_eq!(first, second, "{id}");
            assert_eq!(first.len(), fixture().len());
            assert_ne!(first, fixture(), "{id} must have an observable RGBA result");
        }
    }

    #[test]
    fn canonical_order_is_independent_of_input_order() {
        let effects = json!([
            {"effectType":"stylize.vignette","enabled":true,"params":{"amount":-0.5}},
            {"effectType":"blur.sharpen","enabled":true,"params":{"amount":1.0}}
        ]);
        let prepared = PreparedEffectStack::from_value(Some(&effects)).unwrap();
        assert_eq!(
            prepared
                .effects
                .iter()
                .map(|e| e.effect_type.as_str())
                .collect::<Vec<_>>(),
            vec!["blur.sharpen", "stylize.vignette"]
        );
    }

    #[test]
    fn numeric_parameter_keyframes_bind_by_instance_before_canonical_sorting() {
        let properties = BTreeMap::from([
            (
                "effects".to_string(),
                json!([
                    {
                        "effectInstanceId":"vignette-a",
                        "effectType":"stylize.vignette",
                        "enabled":true,
                        "params":{"amount":-0.2}
                    },
                    {
                        "effectInstanceId":"sharpen-a",
                        "effectType":"blur.sharpen",
                        "enabled":true,
                        "params":{"amount":0.25}
                    },
                    {
                        "effectInstanceId":"sharpen-b",
                        "effectType":"blur.sharpen",
                        "enabled":true,
                        "params":{"amount":0.5}
                    }
                ]),
            ),
            (
                "effectParameterKeyframes".to_string(),
                json!({
                    "vignette-a":{"amount":[
                        {"atSeconds":0.0,"value":-0.2,"easing":"linear"},
                        {"atSeconds":2.0,"value":-0.8,"easing":"linear"}
                    ]},
                    "sharpen-a":{"amount":[
                        {"atSeconds":0.0,"value":0.25,"easing":"easeIn"},
                        {"atSeconds":2.0,"value":1.25,"easing":"linear"}
                    ]},
                    "sharpen-b":{"amount":[
                        {"atSeconds":0.0,"value":0.5,"easing":"hold"},
                        {"atSeconds":2.0,"value":2.0,"easing":"linear"}
                    ]}
                }),
            ),
        ]);

        let prepared = PreparedEffectStack::from_item_properties_for_duration(&properties, 2.0)
            .expect("animated stack");
        assert_eq!(
            prepared
                .effects
                .iter()
                .map(|effect| effect.effect_instance_id.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("sharpen-a"), Some("sharpen-b"), Some("vignette-a")],
            "catalog order is canonical while equal-rank instances retain stored order"
        );

        let sampled = prepared.sample(1.0);
        assert_eq!(sampled.effects[0].params["amount"], 0.5);
        assert_eq!(sampled.effects[1].params["amount"], 0.5);
        assert_eq!(sampled.effects[2].params["amount"], -0.5);
        assert_ne!(
            prepared
                .apply_rgba8_srgb_at_seconds(&fixture(), 2, 2, 7, 0.0)
                .expect("first frame"),
            prepared
                .apply_rgba8_srgb_at_seconds(&fixture(), 2, 2, 7, 2.0)
                .expect("last frame"),
            "rendered pixels must reflect the sampled parameter values"
        );
    }

    #[test]
    fn numeric_parameter_keyframes_fail_closed_for_bad_addresses_and_bounds() {
        let effects = json!([{
            "effectInstanceId":"sharpen-a",
            "effectType":"blur.sharpen",
            "enabled":true,
            "params":{"amount":0.25}
        }]);
        for lanes in [
            json!({"missing":{"amount":[]}}),
            json!({"sharpen-a":{"radius":[]}}),
            json!({"sharpen-a":{"amount":[{"atSeconds":3.0,"value":1.0}]}}),
            json!({"sharpen-a":{"amount":[{"atSeconds":0.0,"value":99.0}]}}),
        ] {
            let properties = BTreeMap::from([
                ("effects".to_string(), effects.clone()),
                ("effectParameterKeyframes".to_string(), lanes),
            ]);
            assert!(matches!(
                PreparedEffectStack::from_item_properties_for_duration(&properties, 2.0),
                Err(EffectStackError::InvalidParameterKeyframes(_))
            ));
        }
    }

    #[test]
    fn typed_curves_validate_and_interpolate_piecewise() {
        let properties = BTreeMap::from([(
            "colorGrade".to_string(),
            json!({
                "masterCurve":[[0.0,0.0],[0.5,0.75],[1.0,1.0]],
                "redCurve":[[0.0,0.0],[1.0,0.5]]
            }),
        )]);
        let stack = PreparedEffectStack::from_item_properties(&properties).expect("curves");
        let output = stack
            .apply_rgba8_srgb(&[128, 128, 128, 77], 1, 1, 0)
            .expect("apply");
        assert_eq!(output, vec![96, 192, 192, 77]);

        let invalid = BTreeMap::from([(
            "colorGrade".to_string(),
            json!({"masterCurve":[[0.5,0.5],[0.4,0.6]]}),
        )]);
        assert!(matches!(
            PreparedEffectStack::from_item_properties(&invalid),
            Err(EffectStackError::InvalidControl { .. })
        ));
    }

    #[test]
    fn color_grade_controls_compile_to_canonical_effect_order() {
        let properties = BTreeMap::from([(
            "colorGrade".to_string(),
            json!({"saturation":0.5,"temperature":8000.0,"tint":10.0,"highlights":0.2}),
        )]);
        let stack = PreparedEffectStack::from_item_properties(&properties).expect("grade");
        assert_eq!(
            stack
                .effects
                .iter()
                .map(|effect| effect.effect_type.as_str())
                .collect::<Vec<_>>(),
            vec![
                "color.highlightsShadows",
                "color.temperature",
                "color.saturation"
            ]
        );
    }

    #[test]
    fn item_effect_curves_and_hue_curves_use_typed_controls() {
        let prepared = PreparedEffectStack::from_value(Some(&json!([
            {
                "effectType":"color.curves",
                "enabled":true,
                "params":{"masterCurve":[[0.0,0.0],[0.5,0.75],[1.0,1.0]]}
            },
            {
                "effectType":"color.hueCurves",
                "enabled":true,
                "params":{"targets":[{"targetHue":350.0,"hueShift":12.0}]}
            }
        ])))
        .expect("catalog-shaped typed effects");

        assert_eq!(
            prepared
                .effects
                .iter()
                .map(|effect| effect.effect_type.as_str())
                .collect::<Vec<_>>(),
            vec!["color.curves", "color.hueCurves"]
        );
        assert!(prepared.effects[0].controls.contains_key("masterCurve"));
        assert_eq!(
            prepared.effects[1].controls["targets"][0]["targetHue"],
            350.0
        );
        assert_ne!(
            prepared
                .apply_rgba8_srgb(&[128, 128, 128, 77], 1, 1, 0)
                .expect("apply typed controls"),
            vec![128, 128, 128, 77]
        );
    }

    #[test]
    fn lut_resource_param_is_accepted_and_excluded_from_pre_lut_stack() {
        let prepared = PreparedEffectStack::from_value(Some(&json!([{
            "effectType":"color.lut",
            "enabled":true,
            "params":{"intensity":0.7,"path":"media/look.cube"}
        }])))
        .expect("LUT catalog resource");

        assert_eq!(prepared.effects.len(), 1);
        assert_eq!(prepared.effects[0].params["intensity"], 0.7);
        assert!(prepared.before_lut().effects.is_empty());
    }

    #[test]
    fn item_effect_typed_controls_reject_unknown_keys() {
        for value in [
            json!([{"effectType":"color.curves","enabled":true,"params":{"unexpected":[]}}]),
            json!([{"effectType":"color.hueCurves","enabled":true,"params":{"targets":[],"unexpected":1}}]),
        ] {
            assert!(matches!(
                PreparedEffectStack::from_value(Some(&value)),
                Err(EffectStackError::UnknownParameter { .. })
            ));
        }
    }

    #[test]
    fn hue_curve_targets_are_canonical_and_wrap_smoothly_across_zero() {
        let targets = json!({"targets":[
            {"targetHue":350.0,"hueShift":-12.0,"satScale":0.8,"lumShift":0.05},
            {"targetHue":10.0,"hueShift":12.0,"satScale":1.2,"lumShift":-0.05}
        ]});
        let reversed = json!({"targets":[
            {"targetHue":10.0,"hueShift":12.0,"satScale":1.2,"lumShift":-0.05},
            {"targetHue":350.0,"hueShift":-12.0,"satScale":0.8,"lumShift":0.05}
        ]});
        let stack_for = |value: Value| {
            PreparedEffectStack::from_item_properties(&BTreeMap::from([(
                "colorGrade".to_string(),
                json!({"hueCurves":value}),
            )]))
            .expect("hue curves")
        };
        let first = stack_for(targets);
        let second = stack_for(reversed);
        assert_eq!(
            first, second,
            "target input order must not affect the fingerprint"
        );
        assert_eq!(first.before_lut(), first, "hue curves execute before LUT");
        assert!((circular_distance(359.0 / 360.0, 1.0 / 360.0) - 2.0 / 360.0).abs() < 1e-12);

        let encode = |hue: f64| {
            let rgb = hsv_to_rgb(hue, 0.8, 0.8);
            [
                clamp_u8(rgb[0] * 255.0),
                clamp_u8(rgb[1] * 255.0),
                clamp_u8(rgb[2] * 255.0),
                91,
            ]
        };
        let left = encode(359.0 / 360.0);
        let right = encode(1.0 / 360.0);
        let left_output = first.apply_rgba8_srgb(&left, 1, 1, 0).expect("left wrap");
        let right_output = first.apply_rgba8_srgb(&right, 1, 1, 0).expect("right wrap");
        assert_ne!(left_output, left);
        assert_ne!(right_output, right);
        assert_eq!(left_output[3], 91);
        assert_eq!(right_output[3], 91);

        let normalized = canonical_hue_curve_targets(&json!({
            "targets":[{"targetHue":360.0}]
        }))
        .expect("360-degree wrap");
        assert_eq!(normalized[0]["targetHue"], 0.0);
        assert!(canonical_hue_curve_targets(&json!({
            "targets":[{"targetHue":361.0}]
        }))
        .is_err());
        assert!(canonical_hue_curve_targets(&json!({
            "targets":[{"targetHue":0.0,"unexpected":1.0}]
        }))
        .is_err());
    }

    #[test]
    fn unsupported_effects_fail_closed() {
        let error = PreparedEffectStack::from_value(Some(
            &json!([{"effectType":"audio.denoise","enabled":true,"params":{}}]),
        ))
        .unwrap_err();
        assert!(matches!(error, EffectStackError::UnsupportedEffect(_)));
    }

    #[test]
    fn golden_rgba_properties_cover_spatial_alpha_and_seeded_effects() {
        assert_eq!(
            stack("blur.gaussian", json!({"radius":1.0}))
                .apply_rgba8_srgb(&fixture(), 2, 2, 0)
                .unwrap(),
            vec![115, 135, 153, 208, 115, 135, 153, 208, 115, 135, 153, 208, 115, 135, 153, 208]
        );
        let keyed = stack(
            "key.chroma",
            json!({"keyHue":0.333,"tolerance":1.0,"softness":0.0,"spill":0.0}),
        )
        .apply_rgba8_srgb(&[0, 255, 0, 255], 1, 1, 0)
        .unwrap();
        assert_eq!(keyed[3], 0);
        let grain_a = stack("stylize.grain", json!({"amount":1.0,"size":1.0}))
            .apply_rgba8_srgb(&fixture(), 2, 2, 3)
            .unwrap();
        let grain_b = stack("stylize.grain", json!({"amount":1.0,"size":1.0}))
            .apply_rgba8_srgb(&fixture(), 2, 2, 4)
            .unwrap();
        assert_ne!(grain_a, grain_b);
    }

    #[test]
    fn full_hd_max_radius_blur_has_linear_time_guard() {
        let width = 1920;
        let height = 1080;
        let source = vec![127_u8; width * height * 4];
        let started = Instant::now();
        let output = box_blur(&source, width as u32, height as u32, 100);
        assert_eq!(output.len(), source.len());
        assert!(started.elapsed() < Duration::from_secs(8));
    }
}
