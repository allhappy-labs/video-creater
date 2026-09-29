use super::model::{TimelineItem, TimelineItemKind};
use serde_json::{json, Map, Value};
use std::fmt::{Display, Formatter};

const TIME_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, PartialEq)]
pub struct AbsoluteOpacityKeyframe {
    pub at_seconds: f64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AbsoluteOpacityCurve {
    base: f64,
    keyframes: Vec<AbsoluteOpacityKeyframe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedOpacityError(pub String);

impl Display for NestedOpacityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for NestedOpacityError {}

impl AbsoluteOpacityCurve {
    pub fn value_at(&self, seconds: f64) -> f64 {
        let Some(first) = self.keyframes.first() else {
            return self.base;
        };
        if seconds < first.at_seconds {
            return self.base;
        }
        for pair in self.keyframes.windows(2) {
            let (left, right) = (&pair[0], &pair[1]);
            if seconds <= right.at_seconds {
                let progress = (seconds - left.at_seconds) / (right.at_seconds - left.at_seconds);
                return left.value + (right.value - left.value) * progress;
            }
        }
        self.keyframes
            .last()
            .map(|keyframe| keyframe.value)
            .unwrap_or(self.base)
    }

    pub fn has_keyframes(&self) -> bool {
        !self.keyframes.is_empty()
    }

    pub fn base(&self) -> f64 {
        self.base
    }
}

pub fn opacity_curve_for_item(
    item: &TimelineItem,
    absolute_start_seconds: f64,
    source_duration_seconds: f64,
) -> Result<AbsoluteOpacityCurve, NestedOpacityError> {
    let base = item
        .properties
        .get("opacity")
        .map(Value::as_f64)
        .unwrap_or(Some(1.0))
        .ok_or_else(|| NestedOpacityError(format!("item `{}` has invalid opacity", item.id)))?;
    if !base.is_finite() || !(0.0..=1.0).contains(&base) {
        return Err(NestedOpacityError(format!(
            "item `{}` has invalid opacity",
            item.id
        )));
    }

    let Some(keyframes) = item.properties.get("keyframes") else {
        return Ok(AbsoluteOpacityCurve {
            base,
            keyframes: Vec::new(),
        });
    };
    let keyframes = keyframes.as_object().ok_or_else(|| {
        NestedOpacityError(format!(
            "item `{}` has malformed opacity keyframes",
            item.id
        ))
    })?;
    let Some(opacity_keyframes) = keyframes.get("opacity") else {
        return Ok(AbsoluteOpacityCurve {
            base,
            keyframes: Vec::new(),
        });
    };
    let opacity_keyframes = opacity_keyframes.as_array().ok_or_else(|| {
        NestedOpacityError(format!(
            "item `{}` has malformed opacity keyframes",
            item.id
        ))
    })?;
    let mut previous_seconds = None;
    let mut parsed = Vec::with_capacity(opacity_keyframes.len());
    for (index, keyframe) in opacity_keyframes.iter().enumerate() {
        let keyframe = keyframe.as_object().ok_or_else(|| {
            NestedOpacityError(format!(
                "item `{}` has malformed opacity keyframe {index}",
                item.id
            ))
        })?;
        let at_seconds = keyframe.get("atSeconds").and_then(Value::as_f64);
        let value = keyframe.get("value").and_then(Value::as_f64);
        let Some((at_seconds, value)) = at_seconds.zip(value) else {
            return Err(NestedOpacityError(format!(
                "item `{}` has malformed opacity keyframe {index}",
                item.id
            )));
        };
        if !at_seconds.is_finite()
            || !value.is_finite()
            || !(0.0..=source_duration_seconds).contains(&at_seconds)
            || !(0.0..=1.0).contains(&value)
            || previous_seconds.is_some_and(|previous| at_seconds <= previous)
        {
            return Err(NestedOpacityError(format!(
                "item `{}` has invalid opacity keyframe {index}",
                item.id
            )));
        }
        previous_seconds = Some(at_seconds);
        parsed.push(AbsoluteOpacityKeyframe {
            at_seconds: absolute_start_seconds + at_seconds,
            value,
        });
    }

    Ok(AbsoluteOpacityCurve {
        base,
        keyframes: parsed,
    })
}

pub fn validate_nested_wrapper_opacity_keyframes(
    item: &TimelineItem,
) -> Result<(), NestedOpacityError> {
    let Some(keyframes) = item.properties.get("keyframes") else {
        return Ok(());
    };
    let keyframes = keyframes.as_object().ok_or_else(|| {
        NestedOpacityError(format!(
            "nested wrapper `{}` has malformed keyframes",
            item.id
        ))
    })?;
    if keyframes.keys().all(|key| key == "opacity") {
        return Ok(());
    }
    Err(NestedOpacityError(format!(
        "nested wrapper `{}` has unsupported keyframes",
        item.id
    )))
}

pub fn fade_opacity_curve_for_item(
    item: &TimelineItem,
    absolute_start_seconds: f64,
    duration_seconds: f64,
) -> Result<Option<AbsoluteOpacityCurve>, NestedOpacityError> {
    let fade_in_seconds = number_property(item, "fadeInSeconds")?.unwrap_or(0.0);
    let fade_out_seconds = number_property(item, "fadeOutSeconds")?.unwrap_or(0.0);
    if fade_in_seconds < 0.0
        || fade_out_seconds < 0.0
        || fade_in_seconds + fade_out_seconds > duration_seconds
    {
        return Err(NestedOpacityError(format!(
            "item `{}` has fades that do not fit inside its duration",
            item.id
        )));
    }
    if fade_in_seconds == 0.0 && fade_out_seconds == 0.0 {
        return Ok(None);
    }
    let mut keyframes = Vec::new();
    if fade_in_seconds > 0.0 {
        keyframes.push(AbsoluteOpacityKeyframe {
            at_seconds: absolute_start_seconds,
            value: 0.0,
        });
        keyframes.push(AbsoluteOpacityKeyframe {
            at_seconds: absolute_start_seconds + fade_in_seconds,
            value: 1.0,
        });
    }
    if fade_out_seconds > 0.0 {
        let fade_out_start = absolute_start_seconds + duration_seconds - fade_out_seconds;
        if keyframes
            .last()
            .is_some_and(|keyframe| (keyframe.at_seconds - fade_out_start).abs() <= TIME_EPSILON)
        {
            // A fade-in and fade-out may meet at the same full-opacity frame.
        } else {
            keyframes.push(AbsoluteOpacityKeyframe {
                at_seconds: fade_out_start,
                value: 1.0,
            });
        }
        keyframes.push(AbsoluteOpacityKeyframe {
            at_seconds: absolute_start_seconds + duration_seconds,
            value: 0.0,
        });
    }
    Ok(Some(AbsoluteOpacityCurve {
        base: 1.0,
        keyframes,
    }))
}

pub fn apply_nested_opacity_curves(
    item: &mut TimelineItem,
    parent_curves: &[AbsoluteOpacityCurve],
    absolute_start_seconds: f64,
    absolute_end_seconds: f64,
    fps: f64,
) -> Result<(), NestedOpacityError> {
    if parent_curves.is_empty() || matches!(item.kind, TimelineItemKind::AudioClip) {
        return Ok(());
    }
    let mut curves = parent_curves.to_vec();
    curves.push(opacity_curve_for_item(
        item,
        absolute_start_seconds,
        item.duration_seconds,
    )?);
    let opacity_at_start = combined_opacity(&curves, absolute_start_seconds);
    item.properties
        .insert("opacity".to_string(), json!(opacity_at_start));

    if !curves.iter().any(AbsoluteOpacityCurve::has_keyframes) {
        remove_opacity_keyframes(item);
        return Ok(());
    }

    let points = sampled_opacity_points(&curves, absolute_start_seconds, absolute_end_seconds, fps);
    let mut keyframes = item
        .properties
        .get("keyframes")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);
    keyframes.insert(
        "opacity".to_string(),
        Value::Array(
            points
                .into_iter()
                .map(|point| {
                    json!({
                        "atSeconds": point.at_seconds - absolute_start_seconds,
                        "value": point.value,
                    })
                })
                .collect(),
        ),
    );
    item.properties
        .insert("keyframes".to_string(), Value::Object(keyframes));
    Ok(())
}

fn combined_opacity(curves: &[AbsoluteOpacityCurve], seconds: f64) -> f64 {
    let value = curves
        .iter()
        .map(|curve| curve.value_at(seconds))
        .product::<f64>()
        .clamp(0.0, 1.0);
    (value * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

fn number_property(item: &TimelineItem, key: &str) -> Result<Option<f64>, NestedOpacityError> {
    let Some(value) = item.properties.get(key) else {
        return Ok(None);
    };
    let value = value
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| NestedOpacityError(format!("item `{}` has invalid {key}", item.id)))?;
    Ok(Some(value))
}

fn sampled_opacity_points(
    curves: &[AbsoluteOpacityCurve],
    start_seconds: f64,
    end_seconds: f64,
    fps: f64,
) -> Vec<AbsoluteOpacityKeyframe> {
    let mut times = vec![start_seconds, end_seconds];
    for curve in curves {
        times.extend(
            curve
                .keyframes
                .iter()
                .map(|keyframe| keyframe.at_seconds)
                .filter(|seconds| *seconds >= start_seconds && *seconds <= end_seconds),
        );
    }
    if fps.is_finite() && fps > 0.0 {
        let start_frame = (start_seconds * fps).ceil() as i64;
        let end_frame = (end_seconds * fps).floor() as i64;
        times.extend((start_frame..=end_frame).map(|frame| frame as f64 / fps));
    }
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() <= TIME_EPSILON);
    times
        .into_iter()
        .map(|at_seconds| AbsoluteOpacityKeyframe {
            at_seconds,
            value: combined_opacity(curves, at_seconds),
        })
        .collect()
}

fn remove_opacity_keyframes(item: &mut TimelineItem) {
    let Some(mut keyframes) = item
        .properties
        .get("keyframes")
        .and_then(Value::as_object)
        .cloned()
    else {
        return;
    };
    keyframes.remove("opacity");
    if keyframes.is_empty() {
        item.properties.remove("keyframes");
    } else {
        item.properties
            .insert("keyframes".to_string(), Value::Object(keyframes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures::sample_project;

    #[test]
    fn samples_multiplied_wrapper_and_child_curves_on_the_frame_grid() {
        let mut item = sample_project().timeline.tracks[0].items[0].clone();
        item.duration_seconds = 1.0;
        item.properties.insert(
            "keyframes".to_string(),
            json!({ "opacity": [
                { "atSeconds": 0.0, "value": 0.8 },
                { "atSeconds": 1.0, "value": 0.4 }
            ], "positionX": [
                { "atSeconds": 0.0, "value": 12.0 },
                { "atSeconds": 1.0, "value": 24.0 }
            ] }),
        );
        let wrapper = AbsoluteOpacityCurve {
            base: 1.0,
            keyframes: vec![
                AbsoluteOpacityKeyframe {
                    at_seconds: 0.0,
                    value: 0.5,
                },
                AbsoluteOpacityKeyframe {
                    at_seconds: 1.0,
                    value: 0.25,
                },
            ],
        };

        apply_nested_opacity_curves(&mut item, &[wrapper], 0.0, 1.0, 4.0)
            .expect("curves should compose");

        assert_eq!(item.properties["opacity"], json!(0.4));
        assert_eq!(
            item.properties["keyframes"]["opacity"],
            json!([
                { "atSeconds": 0.0, "value": 0.4 },
                { "atSeconds": 0.25, "value": 0.30625 },
                { "atSeconds": 0.5, "value": 0.225 },
                { "atSeconds": 0.75, "value": 0.15625 },
                { "atSeconds": 1.0, "value": 0.1 },
            ])
        );
        assert_eq!(
            item.properties["keyframes"]["positionX"],
            json!([
                { "atSeconds": 0.0, "value": 12.0 },
                { "atSeconds": 1.0, "value": 24.0 },
            ])
        );
    }
}
