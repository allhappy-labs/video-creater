use super::ir::{
    AnimationKeyframe, Color, Easing, GraphicNode, LineNode, NodeAnimation, Point, PolygonNode,
    Rect, TextReveal, TextRevealMode, TextRevealOrder,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationState {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub opacity: f64,
    pub rotation_degrees: f64,
    pub blur_radius: f64,
    pub shadow_opacity: f64,
    pub glow_opacity: f64,
    pub clip_progress: f64,
    pub path_progress: f64,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            opacity: 1.0,
            rotation_degrees: 0.0,
            blur_radius: 0.0,
            shadow_opacity: 0.0,
            glow_opacity: 0.0,
            clip_progress: 1.0,
            path_progress: 1.0,
        }
    }
}

pub fn layer_has_animation(nodes: &[GraphicNode]) -> bool {
    nodes.iter().any(|node| {
        node.animation()
            .is_some_and(|animation| !animation.keyframes.is_empty())
            || matches!(node, GraphicNode::Text(node) if node.text_reveal.is_some())
    })
}

pub fn frame_count_for_duration(duration_seconds: f64, fps: f64, animated: bool) -> u32 {
    if !animated {
        return 1;
    }

    (duration_seconds * fps).ceil().max(1.0) as u32
}

pub fn animate_node(node: &GraphicNode, time_seconds: f64, duration_seconds: f64) -> GraphicNode {
    let animation = node.animation();
    let has_text_reveal = matches!(node, GraphicNode::Text(node) if node.text_reveal.is_some());
    if animation.is_none() && !has_text_reveal {
        return node.clone();
    };
    let state = animation
        .map(|animation| evaluate_animation(animation, time_seconds, duration_seconds))
        .unwrap_or_default();

    match node {
        GraphicNode::Text(node) => {
            let mut node = node.clone();
            if let Some(reveal) = &node.text_reveal {
                node.text = reveal_text(&node.text, reveal, time_seconds, duration_seconds);
            }
            node.box_rect = transform_rect(&node.box_rect, state);
            node.font_size = (node.font_size * state.scale_x.min(state.scale_y)).max(1.0);
            node.fill = multiply_color_alpha(&node.fill, state.opacity);
            GraphicNode::Text(node)
        }
        GraphicNode::RoundedRect(node) => {
            let mut node = node.clone();
            node.box_rect = transform_rect(&node.box_rect, state);
            node.radius = (node.radius * state.scale_x.min(state.scale_y)).max(0.0);
            node.fill = multiply_color_alpha(&node.fill, state.opacity);
            GraphicNode::RoundedRect(node)
        }
        GraphicNode::Rect(node) => {
            let mut node = node.clone();
            node.box_rect = transform_rect(&node.box_rect, state);
            node.fill = multiply_color_alpha(&node.fill, state.opacity);
            GraphicNode::Rect(node)
        }
        GraphicNode::Polygon(node) => GraphicNode::Polygon(transform_polygon(node, state)),
        GraphicNode::Line(node) => GraphicNode::Line(transform_line(node, state)),
        GraphicNode::ImageRef(node) => {
            let mut node = node.clone();
            node.box_rect = transform_rect(&node.box_rect, state);
            node.opacity = (node.opacity * state.opacity).clamp(0.0, 1.0);
            GraphicNode::ImageRef(node)
        }
        GraphicNode::HolographicLogo(node) => {
            let mut node = node.clone();
            node.box_rect = transform_rect(&node.box_rect, state);
            node.opacity = (node.opacity * state.opacity).clamp(0.0, 1.0);
            GraphicNode::HolographicLogo(node)
        }
    }
}

/// Resolves one node at a specific source-layer time and removes its animation
/// metadata so a one-frame range render cannot replay the animation from zero.
pub fn bake_node_at_time(
    node: &GraphicNode,
    time_seconds: f64,
    duration_seconds: f64,
) -> GraphicNode {
    let mut node = animate_node(node, time_seconds, duration_seconds);
    match &mut node {
        GraphicNode::Text(node) => {
            node.animate = None;
            node.text_reveal = None;
        }
        GraphicNode::RoundedRect(node) => node.animate = None,
        GraphicNode::Rect(node) => node.animate = None,
        GraphicNode::Polygon(node) => node.animate = None,
        GraphicNode::Line(node) => node.animate = None,
        GraphicNode::ImageRef(node) => node.animate = None,
        GraphicNode::HolographicLogo(node) => node.animate = None,
    }
    node
}

fn reveal_text(
    text: &str,
    reveal: &TextReveal,
    time_seconds: f64,
    duration_seconds: f64,
) -> String {
    let units = reveal_units(text, reveal.mode);
    if units.is_empty() {
        return String::new();
    }
    if matches!(reveal.mode, TextRevealMode::Whole) {
        return if time_seconds > 0.0 || duration_seconds <= 0.0 {
            text.to_string()
        } else {
            String::new()
        };
    }

    let max_duration = reveal
        .max_reveal_duration_seconds
        .unwrap_or_else(|| (duration_seconds * 0.3).clamp(0.1, 0.45));
    let stagger = reveal.stagger_seconds.unwrap_or_else(|| {
        if units.len() <= 1 {
            max_duration
        } else {
            max_duration / (units.len() - 1) as f64
        }
    });
    let visible_count = if stagger <= f64::EPSILON {
        units.len()
    } else {
        ((time_seconds.max(0.0) / stagger).floor() as usize + 1).min(units.len())
    };
    let visible_count = if time_seconds >= max_duration {
        units.len()
    } else {
        visible_count
    };

    let mut visible = vec![false; units.len()];
    match reveal.order.unwrap_or(TextRevealOrder::Forward) {
        TextRevealOrder::Forward => {
            visible
                .iter_mut()
                .take(visible_count)
                .for_each(|slot| *slot = true);
        }
        TextRevealOrder::Reverse => {
            visible
                .iter_mut()
                .rev()
                .take(visible_count)
                .for_each(|slot| *slot = true);
        }
        TextRevealOrder::CenterOut => {
            for index in center_out_indexes(units.len())
                .into_iter()
                .take(visible_count)
            {
                visible[index] = true;
            }
        }
    }

    units
        .into_iter()
        .enumerate()
        .filter_map(|(index, unit)| visible[index].then_some(unit))
        .collect::<Vec<_>>()
        .join(match reveal.mode {
            TextRevealMode::Word => " ",
            _ => "",
        })
}

fn reveal_units(text: &str, mode: TextRevealMode) -> Vec<String> {
    match mode {
        TextRevealMode::Whole => vec![text.to_string()],
        TextRevealMode::Line => text.split_inclusive('\n').map(str::to_string).collect(),
        TextRevealMode::Word => text.split_whitespace().map(str::to_string).collect(),
        TextRevealMode::Character => text
            .chars()
            .map(|character| character.to_string())
            .collect(),
    }
}

fn center_out_indexes(len: usize) -> Vec<usize> {
    if len == 0 {
        return Vec::new();
    }
    let center = (len - 1) / 2;
    let mut indexes = vec![center];
    let mut offset = 1;
    while indexes.len() < len {
        if center >= offset {
            indexes.push(center - offset);
        }
        let right = center + offset;
        if right < len {
            indexes.push(right);
        }
        offset += 1;
    }
    indexes
}

pub fn evaluate_animation(
    animation: &NodeAnimation,
    time_seconds: f64,
    duration_seconds: f64,
) -> AnimationState {
    if animation.keyframes.is_empty() || duration_seconds <= 0.0 {
        return AnimationState::default();
    }

    let progress = animation_progress(animation, time_seconds, duration_seconds);
    let easing = animation.ease.unwrap_or(Easing::Linear);

    AnimationState {
        x: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.x,
            0.0,
        ),
        y: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.y,
            0.0,
        ),
        scale_x: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.scale_x.or(keyframe.scale),
            1.0,
        ),
        scale_y: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.scale_y.or(keyframe.scale),
            1.0,
        ),
        opacity: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.opacity,
            1.0,
        )
        .clamp(0.0, 1.0),
        rotation_degrees: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.rotation_degrees,
            0.0,
        ),
        blur_radius: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.blur_radius,
            0.0,
        )
        .max(0.0),
        shadow_opacity: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.shadow_opacity,
            0.0,
        )
        .clamp(0.0, 1.0),
        glow_opacity: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.glow_opacity,
            0.0,
        )
        .clamp(0.0, 1.0),
        clip_progress: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.clip_progress,
            1.0,
        )
        .clamp(0.0, 1.0),
        path_progress: interpolate_property(
            &animation.keyframes,
            progress,
            easing,
            |keyframe| keyframe.path_progress,
            1.0,
        )
        .clamp(0.0, 1.0),
    }
}

fn animation_progress(animation: &NodeAnimation, time_seconds: f64, duration_seconds: f64) -> f64 {
    let delay = animation.delay_seconds.unwrap_or(0.0).max(0.0);
    let active_duration = (duration_seconds - delay).max(f64::EPSILON);
    let repeat = animation.repeat.unwrap_or(0);
    let total_cycles = repeat.saturating_add(1) as f64;
    let cycle_duration = active_duration / total_cycles;
    let raw = ((time_seconds - delay) / cycle_duration.max(f64::EPSILON)).max(0.0);
    if raw >= total_cycles {
        return final_progress(animation, repeat);
    }

    let cycle = raw.floor() as u32;
    let progress = raw.fract();
    if animation.yoyo.unwrap_or(false) && cycle % 2 == 1 {
        1.0 - progress
    } else {
        progress
    }
}

fn final_progress(animation: &NodeAnimation, repeat: u32) -> f64 {
    if animation.yoyo.unwrap_or(false) && repeat % 2 == 1 {
        0.0
    } else {
        1.0
    }
}

fn interpolate_property(
    keyframes: &[AnimationKeyframe],
    progress: f64,
    easing: Easing,
    value_for_keyframe: impl Fn(&AnimationKeyframe) -> Option<f64>,
    fallback: f64,
) -> f64 {
    let mut previous = None;
    for keyframe in keyframes {
        let Some(value) = value_for_keyframe(keyframe) else {
            continue;
        };

        if keyframe.at <= progress {
            previous = Some((keyframe.at, value));
            continue;
        }

        let (previous_at, previous_value) = previous.unwrap_or((0.0, fallback));
        let span = (keyframe.at - previous_at).max(f64::EPSILON);
        let local_t = ((progress - previous_at) / span).clamp(0.0, 1.0);
        let eased_t = apply_ease(easing, local_t);
        return previous_value + (value - previous_value) * eased_t;
    }

    if let Some((_, value)) = previous {
        value
    } else {
        fallback
    }
}

fn apply_ease(easing: Easing, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    match easing {
        Easing::Linear => t,
        Easing::InQuad => t * t,
        Easing::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::InOutQuad => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        Easing::InCubic => t * t * t,
        Easing::OutCubic => 1.0 - (1.0 - t).powi(3),
        Easing::InOutCubic => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
        Easing::OutBack => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
        Easing::OutElastic => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                let c4 = (2.0 * std::f64::consts::PI) / 3.0;
                2_f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
            }
        }
    }
}

fn transform_rect(rect: &Rect, state: AnimationState) -> Rect {
    let width = rect.width * state.scale_x;
    let height = rect.height * state.scale_y;
    Rect {
        x: rect.x + state.x - (width - rect.width) / 2.0,
        y: rect.y + state.y - (height - rect.height) / 2.0,
        width,
        height,
    }
}

fn transform_line(node: &LineNode, state: AnimationState) -> LineNode {
    let center = points_center(&node.points);
    let mut node = node.clone();
    node.points = transform_points(&node.points, center, state);
    node.stroke = multiply_color_alpha(&node.stroke, state.opacity);
    node.stroke_width *= state.scale_x.min(state.scale_y).max(0.0);
    node
}

fn transform_polygon(node: &PolygonNode, state: AnimationState) -> PolygonNode {
    let center = points_center(&node.points);
    let mut node = node.clone();
    node.points = transform_points(&node.points, center, state);
    node.fill = multiply_color_alpha(&node.fill, state.opacity);
    node.stroke = node
        .stroke
        .as_ref()
        .map(|stroke| multiply_color_alpha(stroke, state.opacity));
    node.stroke_width = node
        .stroke_width
        .map(|stroke_width| stroke_width * state.scale_x.min(state.scale_y).max(0.0));
    node
}

fn transform_points(points: &[Point], center: Point, state: AnimationState) -> Vec<Point> {
    points
        .iter()
        .map(|point| Point {
            x: center.x + (point.x - center.x) * state.scale_x + state.x,
            y: center.y + (point.y - center.y) * state.scale_y + state.y,
        })
        .collect()
}

fn points_center(points: &[Point]) -> Point {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    Point {
        x: (min_x + max_x) / 2.0,
        y: (min_y + max_y) / 2.0,
    }
}

fn multiply_color_alpha(color: &Color, opacity: f64) -> Color {
    let Color::Hex(raw) = color;
    let opacity = opacity.clamp(0.0, 1.0);
    let Some(hex) = raw.strip_prefix('#') else {
        return color.clone();
    };
    if hex.len() != 6 && hex.len() != 8 {
        return color.clone();
    }

    let base_alpha = if hex.len() == 8 {
        u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
    } else {
        255
    };
    let alpha = ((base_alpha as f64 * opacity).round()).clamp(0.0, 255.0) as u8;
    Color::Hex(format!("#{}{:02X}", &hex[..6], alpha))
}
