use super::animation::{frame_count_for_duration, layer_has_animation};
use super::assets::AssetRegistry;
use super::error::{ActionableError, ActionableResult, GraphicsErrorCode};
use super::ir::{
    AnimationKeyframe, Color, GraphicNode, GraphicRole, GraphicsLayer, NodeAnimation, Point, Rect,
    TextReveal, TextRevealMode, TransformOriginCoordinate,
};
use std::collections::BTreeSet;

const MIN_DIMENSION: u32 = 16;
const MAX_DIMENSION: u32 = 7680;
const MIN_FPS: f64 = 1.0;
const MAX_FPS: f64 = 120.0;
const MAX_DURATION_SECONDS: f64 = 600.0;
const MIN_READABLE_FONT_SIZE: f64 = 14.0;
const MAX_ANIMATED_FRAME_COUNT: u32 = 1_800;
const MAX_ANIMATED_PIXEL_FRAMES: u64 = 1920 * 1080 * MAX_ANIMATED_FRAME_COUNT as u64;
const MAX_ANIMATION_REPEAT: u32 = 64;

pub fn validate_graphics_layer(layer: &GraphicsLayer) -> ActionableResult<()> {
    let mut errors = Vec::new();

    if layer.schema_version != 1 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsSchemaUnsupported,
            "schemaVersion",
            "Graphics schema version is not supported.",
            "Set schemaVersion to 1.",
        ));
    }

    if layer.id.trim().is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsEmptyNodeId,
            "id",
            "Layer id is empty.",
            "Set id to a stable non-empty string.",
        ));
    }

    validate_dimensions(layer, &mut errors);
    validate_timing(layer, &mut errors);
    validate_required_text(layer, &mut errors);
    validate_nodes(layer, &mut errors);
    validate_frame_budget(layer, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_frame_budget(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if !layer_has_animation(&layer.nodes)
        || !layer.duration_seconds.is_finite()
        || !layer.fps.is_finite()
        || layer.duration_seconds <= 0.0
        || layer.fps <= 0.0
    {
        return;
    }

    let frame_count = frame_count_for_duration(layer.duration_seconds, layer.fps, true);
    let pixel_frames =
        layer.dimensions.width as u64 * layer.dimensions.height as u64 * frame_count as u64;
    if frame_count > MAX_ANIMATED_FRAME_COUNT || pixel_frames > MAX_ANIMATED_PIXEL_FRAMES {
        errors.push(
            ActionableError::new(
                GraphicsErrorCode::GraphicsFrameCoverageExceeded,
                "frames",
                "Animated graphics frame budget is too large.",
                "Reduce durationSeconds, fps, dimensions, or animated nodes.",
            )
            .with_detail("frameCount", frame_count.to_string())
            .with_detail("maxFrameCount", MAX_ANIMATED_FRAME_COUNT.to_string()),
        );
    }
}

pub fn validate_graphics_layer_with_assets(
    layer: &GraphicsLayer,
    assets: &AssetRegistry,
) -> ActionableResult<()> {
    let mut errors = match validate_graphics_layer(layer) {
        Ok(()) => Vec::new(),
        Err(errors) => errors,
    };

    for (index, node) in layer.nodes.iter().enumerate() {
        if let GraphicNode::ImageRef(image) = node {
            if let Err(asset_errors) = assets.resolve(&image.asset_id) {
                let path = format!("nodes[{index}].assetId");
                errors.extend(asset_errors.into_iter().map(|mut error| {
                    error.path = path.clone();
                    error
                }));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_dimensions(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if layer.dimensions.width < MIN_DIMENSION
        || layer.dimensions.height < MIN_DIMENSION
        || layer.dimensions.width > MAX_DIMENSION
        || layer.dimensions.height > MAX_DIMENSION
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            "dimensions",
            "Layer dimensions are outside the supported range.",
            "Use width and height between 16 and 7680 pixels.",
        ));
    }
}

fn validate_timing(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if !layer.timeline_start.is_finite() || layer.timeline_start < 0.0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "timelineStart",
            "Layer start time is invalid.",
            "Use a finite timelineStart greater than or equal to 0.",
        ));
    }
    if !layer.duration_seconds.is_finite()
        || layer.duration_seconds <= 0.0
        || layer.duration_seconds > MAX_DURATION_SECONDS
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "durationSeconds",
            "Layer duration is invalid.",
            "Use a finite durationSeconds between 0 and 600.",
        ));
    }
    if !layer.fps.is_finite() || !(MIN_FPS..=MAX_FPS).contains(&layer.fps) {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            "fps",
            "Layer fps is invalid.",
            "Use a finite fps between 1 and 120.",
        ));
    }
}

fn validate_required_text(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    for (field, value) in [
        ("sourceBeat", &layer.source_beat),
        ("visualTreatment", &layer.visual_treatment),
        ("motion", &layer.motion),
        ("safeZone", &layer.safe_zone),
        ("avoid", &layer.avoid),
    ] {
        if value.trim().is_empty() {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextEmpty,
                field,
                "Required visual metadata is empty.",
                format!("Set {field} to a concise non-empty description."),
            ));
        }
    }
}

fn validate_nodes(layer: &GraphicsLayer, errors: &mut Vec<ActionableError>) {
    if layer.nodes.is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsUnsupportedPrimitive,
            "nodes",
            "Layer has no drawable nodes.",
            "Add at least one supported primitive node.",
        ));
    }

    let mut ids = BTreeSet::new();
    for (index, node) in layer.nodes.iter().enumerate() {
        let path = format!("nodes[{index}]");
        let id = node.id().trim();
        if id.is_empty() || !ids.insert(id.to_string()) {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsEmptyNodeId,
                format!("{path}.id"),
                "Node id is empty or duplicated.",
                "Use a unique non-empty id for every node.",
            ));
        }

        match node {
            GraphicNode::Text(text) => {
                validate_rect(&text.box_rect, &format!("{path}.box"), errors);
                validate_color(&text.fill, &format!("{path}.fill"), errors);
                validate_animation(
                    text.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
                validate_text_reveal(
                    text.text_reveal.as_ref(),
                    &format!("{path}.textReveal"),
                    &layer.role,
                    layer.duration_seconds,
                    errors,
                );
                validate_text_emphasis(text, &format!("{path}.emphasis"), errors);
                if text.text.trim().is_empty() {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTextEmpty,
                        format!("{path}.text"),
                        "Text node content is empty.",
                        "Set text to a short readable string.",
                    ));
                }
                if !text.font_size.is_finite() || text.font_size < MIN_READABLE_FONT_SIZE {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTextUnreadable,
                        format!("{path}.fontSize"),
                        "Text font size is too small.",
                        "Use fontSize greater than or equal to 14.",
                    ));
                }
                if text.box_rect.width < text.font_size * 0.8 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTextOverflow,
                        format!("{path}.box"),
                        "Text box is too narrow for the configured font size.",
                        "Reduce fontSize, shorten text, or increase box width.",
                    ));
                }
            }
            GraphicNode::RoundedRect(rect) => {
                validate_rect(&rect.box_rect, &format!("{path}.box"), errors);
                validate_radius(rect.radius, &format!("{path}.radius"), errors);
                validate_color(&rect.fill, &format!("{path}.fill"), errors);
                validate_animation(
                    rect.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
            }
            GraphicNode::Rect(rect) => {
                validate_rect(&rect.box_rect, &format!("{path}.box"), errors);
                validate_color(&rect.fill, &format!("{path}.fill"), errors);
                validate_animation(
                    rect.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
            }
            GraphicNode::Polygon(polygon) => {
                validate_animation(
                    polygon.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
                if polygon.points.len() < 3 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsUnsupportedPrimitive,
                        format!("{path}.points"),
                        "Polygon node needs at least three points.",
                        "Add three or more finite points.",
                    ));
                }
                for (point_index, point) in polygon.points.iter().enumerate() {
                    validate_point(point, &format!("{path}.points[{point_index}]"), errors);
                }
                validate_color(&polygon.fill, &format!("{path}.fill"), errors);
                if let Some(stroke) = &polygon.stroke {
                    validate_color(stroke, &format!("{path}.stroke"), errors);
                }
                if let Some(stroke_width) = polygon.stroke_width {
                    validate_stroke_width(stroke_width, &format!("{path}.strokeWidth"), errors);
                }
            }
            GraphicNode::Line(line) => {
                validate_animation(
                    line.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
                if line.points.len() < 2 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsUnsupportedPrimitive,
                        format!("{path}.points"),
                        "Line node needs at least two points.",
                        "Add two or more finite points.",
                    ));
                }
                for (point_index, point) in line.points.iter().enumerate() {
                    validate_point(point, &format!("{path}.points[{point_index}]"), errors);
                }
                validate_color(&line.stroke, &format!("{path}.stroke"), errors);
                validate_stroke_width(line.stroke_width, &format!("{path}.strokeWidth"), errors);
            }
            GraphicNode::ImageRef(image) => {
                validate_rect(&image.box_rect, &format!("{path}.box"), errors);
                validate_animation(
                    image.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
                if image.asset_id.trim().is_empty() {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsImageRefMissing,
                        format!("{path}.assetId"),
                        "Image asset reference is empty.",
                        "Set assetId to an approved project or bundled image asset.",
                    ));
                }
                if !image.opacity.is_finite() || image.opacity < 0.0 || image.opacity > 1.0 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTemplateParamInvalid,
                        format!("{path}.opacity"),
                        "Image opacity is invalid.",
                        "Use opacity between 0 and 1.",
                    ));
                }
            }
            GraphicNode::HolographicLogo(logo) => {
                validate_rect(&logo.view_box, &format!("{path}.viewBox"), errors);
                validate_rect(&logo.box_rect, &format!("{path}.box"), errors);
                validate_animation(
                    logo.animate.as_ref(),
                    &format!("{path}.animate"),
                    layer.duration_seconds,
                    errors,
                );
                if logo.asset_id.trim().is_empty() {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsImageRefMissing,
                        format!("{path}.assetId"),
                        "Holographic logo asset reference is empty.",
                        "Set assetId to an approved built-in logo asset.",
                    ));
                }
                if logo.path_data.is_empty()
                    || logo
                        .path_data
                        .iter()
                        .any(|path_data| path_data.trim().is_empty())
                {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTemplateParamInvalid,
                        format!("{path}.pathData"),
                        "Holographic logo path data is empty.",
                        "Provide at least one non-empty SVG path string for the approved logo.",
                    ));
                }
                if !logo.opacity.is_finite() || logo.opacity < 0.0 || logo.opacity > 1.0 {
                    errors.push(ActionableError::new(
                        GraphicsErrorCode::GraphicsTemplateParamInvalid,
                        format!("{path}.opacity"),
                        "Holographic logo opacity is invalid.",
                        "Use opacity between 0 and 1.",
                    ));
                }
            }
        }
    }
}

fn validate_text_emphasis(
    text: &super::ir::TextNode,
    path: &str,
    errors: &mut Vec<ActionableError>,
) {
    let mut previous_end = 0;
    for (index, emphasis) in text.emphasis.iter().enumerate() {
        let range_path = format!("{path}[{index}]");
        let valid_range = emphasis.start_byte < emphasis.end_byte
            && emphasis.end_byte <= text.text.len()
            && text.text.is_char_boundary(emphasis.start_byte)
            && text.text.is_char_boundary(emphasis.end_byte)
            && emphasis.start_byte >= previous_end;
        if !valid_range {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextOverflow,
                range_path,
                "Text emphasis ranges must be ordered, non-empty UTF-8 byte ranges inside the text.",
                "Use ordered word ranges that fall inside the text node.",
            ));
            continue;
        }
        previous_end = emphasis.end_byte;
        validate_color(&emphasis.fill, &format!("{path}[{index}].fill"), errors);
        match (emphasis.active_start_seconds, emphasis.active_end_seconds) {
            (Some(start), Some(end))
                if start.is_finite() && end.is_finite() && start >= 0.0 && end > start => {}
            (None, None) => {}
            _ => errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextOverflow,
                format!("{path}[{index}]"),
                "Text emphasis activity timing must be a finite positive range.",
                "Set both activeStartSeconds and activeEndSeconds, with end after start.",
            )),
        }
        match (
            emphasis.enter_start_seconds,
            emphasis.enter_end_seconds,
            emphasis.hold_end_seconds,
            emphasis.exit_end_seconds,
        ) {
            (Some(enter_start), Some(enter_end), Some(hold_end), Some(exit_end))
                if [enter_start, enter_end, hold_end, exit_end]
                    .iter()
                    .all(|value| value.is_finite())
                    && 0.0 <= enter_start
                    && enter_start <= enter_end
                    && enter_end <= hold_end
                    && hold_end <= exit_end => {}
            (None, None, None, None) => {}
            _ => errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextOverflow,
                format!("{path}[{index}]"),
                "Text emphasis enter, hold, and exit timing must be complete and ordered.",
                "Keep enterStart <= enterEnd <= holdEnd <= exitEnd.",
            )),
        }
        if emphasis
            .emphasis_scale
            .is_some_and(|value| !value.is_finite() || !(0.5..=2.0).contains(&value))
            || emphasis
                .emphasis_opacity
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextUnreadable,
                format!("{path}[{index}]"),
                "Text emphasis scale or opacity is outside supported bounds.",
                "Use scale from 0.5 through 2.0 and opacity from 0 through 1.",
            ));
        }
        if emphasis.font_weight == 0 {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTextUnreadable,
                format!("{path}[{index}].fontWeight"),
                "Text emphasis font weight must be positive.",
                "Use a positive font weight.",
            ));
        }
    }
}

fn validate_stroke_width(stroke_width: f64, path: &str, errors: &mut Vec<ActionableError>) {
    if !stroke_width.is_finite() || stroke_width <= 0.0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            path,
            "Stroke width is invalid.",
            "Use a finite strokeWidth greater than 0.",
        ));
    }
}

fn validate_animation(
    animation: Option<&NodeAnimation>,
    path: &str,
    layer_duration_seconds: f64,
    errors: &mut Vec<ActionableError>,
) {
    let Some(animation) = animation else {
        return;
    };

    if animation.keyframes.is_empty() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            format!("{path}.keyframes"),
            "Animation has no keyframes.",
            "Add at least one keyframe with at between 0 and 1.",
        ));
    }

    if let Some(delay) = animation.delay_seconds {
        if !delay.is_finite() || delay < 0.0 {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsInvalidTiming,
                format!("{path}.delaySeconds"),
                "Animation delaySeconds is invalid.",
                "Use a finite delaySeconds greater than or equal to 0.",
            ));
        } else if layer_duration_seconds.is_finite()
            && layer_duration_seconds > 0.0
            && delay >= layer_duration_seconds
        {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsInvalidTiming,
                format!("{path}.delaySeconds"),
                "Animation delaySeconds leaves no active duration.",
                "Use delaySeconds lower than the layer durationSeconds.",
            ));
        }
    }

    if animation
        .repeat
        .is_some_and(|repeat| repeat > MAX_ANIMATION_REPEAT)
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            format!("{path}.repeat"),
            "Animation repeat is too high.",
            format!("Use repeat between 0 and {MAX_ANIMATION_REPEAT}."),
        ));
    }

    if let Some(origin) = &animation.origin {
        validate_origin_coordinate(&origin.x, "x", &format!("{path}.origin.x"), errors);
        validate_origin_coordinate(&origin.y, "y", &format!("{path}.origin.y"), errors);
    }

    let mut previous_at = None;
    for (index, keyframe) in animation.keyframes.iter().enumerate() {
        let keyframe_path = format!("{path}.keyframes[{index}]");
        validate_animation_keyframe(keyframe, &keyframe_path, errors);
        if let Some(previous) = previous_at {
            if keyframe.at <= previous {
                errors.push(ActionableError::new(
                    GraphicsErrorCode::GraphicsInvalidTiming,
                    format!("{keyframe_path}.at"),
                    "Animation keyframes are not strictly increasing.",
                    "Sort keyframes by unique ascending at values between 0 and 1.",
                ));
            }
        }
        previous_at = Some(keyframe.at);
    }
}

fn validate_animation_keyframe(
    keyframe: &AnimationKeyframe,
    path: &str,
    errors: &mut Vec<ActionableError>,
) {
    if !keyframe.at.is_finite() || !(0.0..=1.0).contains(&keyframe.at) {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidTiming,
            format!("{path}.at"),
            "Animation keyframe at is invalid.",
            "Use a finite keyframe at value between 0 and 1.",
        ));
    }

    let mut has_property = false;
    for (field, value) in [
        ("x", keyframe.x),
        ("y", keyframe.y),
        ("scale", keyframe.scale),
        ("scaleX", keyframe.scale_x),
        ("scaleY", keyframe.scale_y),
        ("opacity", keyframe.opacity),
        ("rotationDegrees", keyframe.rotation_degrees),
        ("blurRadius", keyframe.blur_radius),
        ("shadowOpacity", keyframe.shadow_opacity),
        ("glowOpacity", keyframe.glow_opacity),
        ("clipProgress", keyframe.clip_progress),
        ("pathProgress", keyframe.path_progress),
    ] {
        let Some(value) = value else {
            continue;
        };
        has_property = true;
        if !value.is_finite() {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTemplateParamInvalid,
                format!("{path}.{field}"),
                "Animation keyframe value is not finite.",
                "Use finite numeric animation values.",
            ));
        }
    }

    for (field, value) in [
        ("scale", keyframe.scale),
        ("scaleX", keyframe.scale_x),
        ("scaleY", keyframe.scale_y),
    ] {
        if value.is_some_and(|value| value <= 0.0) {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTemplateParamInvalid,
                format!("{path}.{field}"),
                "Animation scale value is invalid.",
                "Use scale values greater than 0.",
            ));
        }
    }

    if keyframe
        .opacity
        .is_some_and(|opacity| !(0.0..=1.0).contains(&opacity))
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            format!("{path}.opacity"),
            "Animation opacity is invalid.",
            "Use opacity between 0 and 1.",
        ));
    }

    if keyframe
        .rotation_degrees
        .is_some_and(|rotation| !(-45.0..=45.0).contains(&rotation))
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            format!("{path}.rotationDegrees"),
            "Animation rotationDegrees is invalid.",
            "Use rotationDegrees between -45 and 45 for overlay graphics.",
        ));
    }

    if keyframe
        .blur_radius
        .is_some_and(|blur| !(0.0..=64.0).contains(&blur))
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            format!("{path}.blurRadius"),
            "Animation blurRadius is invalid.",
            "Use blurRadius between 0 and 64 pixels.",
        ));
    }

    for (field, value) in [
        ("shadowOpacity", keyframe.shadow_opacity),
        ("glowOpacity", keyframe.glow_opacity),
        ("clipProgress", keyframe.clip_progress),
        ("pathProgress", keyframe.path_progress),
    ] {
        if value.is_some_and(|value| !(0.0..=1.0).contains(&value)) {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTemplateParamInvalid,
                format!("{path}.{field}"),
                "Animation progress or opacity value is invalid.",
                format!("Use {field} between 0 and 1."),
            ));
        }
    }

    if !has_property {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            path,
            "Animation keyframe has no animated properties.",
            "Set at least one supported animation property such as x, y, scale, opacity, rotationDegrees, clipProgress, or pathProgress.",
        ));
    }
}

fn validate_origin_coordinate(
    coordinate: &TransformOriginCoordinate,
    axis: &str,
    path: &str,
    errors: &mut Vec<ActionableError>,
) {
    match coordinate {
        TransformOriginCoordinate::Pixels(value) => {
            if !value.is_finite() {
                errors.push(ActionableError::new(
                    GraphicsErrorCode::GraphicsTemplateParamInvalid,
                    path,
                    "Animation transform origin is not finite.",
                    "Use a finite pixel coordinate or an allowed origin keyword.",
                ));
            }
        }
        TransformOriginCoordinate::Keyword(keyword) => {
            let allowed = match axis {
                "x" => ["left", "center", "right"],
                _ => ["top", "center", "bottom"],
            };
            if !allowed.contains(&keyword.as_str()) {
                errors.push(ActionableError::new(
                    GraphicsErrorCode::GraphicsTemplateParamInvalid,
                    path,
                    "Animation transform origin keyword is invalid.",
                    format!("Use one of {} for origin.{axis}.", allowed.join(", ")),
                ));
            }
        }
    }
}

fn validate_text_reveal(
    reveal: Option<&TextReveal>,
    path: &str,
    role: &GraphicRole,
    layer_duration_seconds: f64,
    errors: &mut Vec<ActionableError>,
) {
    let Some(reveal) = reveal else {
        return;
    };

    if *role == GraphicRole::Caption && reveal.mode == TextRevealMode::Character {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            format!("{path}.mode"),
            "Character text reveal is not allowed for ordinary captions.",
            "Use whole, line, or word reveal for captions, or reserve character reveal for title cards and chapter cards.",
        ));
    }

    if reveal
        .stagger_seconds
        .is_some_and(|stagger| !stagger.is_finite() || stagger < 0.0)
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsTemplateParamInvalid,
            format!("{path}.staggerSeconds"),
            "Text reveal staggerSeconds is invalid.",
            "Use a finite staggerSeconds greater than or equal to 0.",
        ));
    }

    if let Some(max_duration) = reveal.max_reveal_duration_seconds {
        let max_allowed = if layer_duration_seconds.is_finite() && layer_duration_seconds > 0.0 {
            layer_duration_seconds / 2.0
        } else {
            0.0
        };
        if !max_duration.is_finite() || max_duration <= 0.0 || max_duration > max_allowed {
            errors.push(ActionableError::new(
                GraphicsErrorCode::GraphicsTemplateParamInvalid,
                format!("{path}.maxRevealDurationSeconds"),
                "Text reveal maxRevealDurationSeconds leaves too little readable hold time.",
                "Use a finite positive reveal duration no longer than half the layer duration.",
            ));
        }
    }
}

fn validate_radius(radius: f64, path: &str, errors: &mut Vec<ActionableError>) {
    if !radius.is_finite() || radius < 0.0 {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            path,
            "Rounded rectangle radius is invalid.",
            "Use a finite radius greater than or equal to 0.",
        ));
    }
}

fn validate_point(point: &Point, path: &str, errors: &mut Vec<ActionableError>) {
    if !point.x.is_finite() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            format!("{path}.x"),
            "Line point x coordinate is invalid.",
            "Use finite x and y coordinates for every line point.",
        ));
    }
    if !point.y.is_finite() {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            format!("{path}.y"),
            "Line point y coordinate is invalid.",
            "Use finite x and y coordinates for every line point.",
        ));
    }
}

fn validate_color(color: &Color, path: &str, errors: &mut Vec<ActionableError>) {
    let Color::Hex(value) = color;
    if !is_valid_hex_color(value) {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidColor,
            path,
            "Color is not a supported hex value.",
            "Use #RRGGBB or #RRGGBBAA.",
        ));
    }
}

fn is_valid_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    matches!(bytes.len(), 7 | 9) && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

fn validate_rect(rect: &Rect, path: &str, errors: &mut Vec<ActionableError>) {
    if !rect.x.is_finite()
        || !rect.y.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        errors.push(ActionableError::new(
            GraphicsErrorCode::GraphicsInvalidDimensions,
            path,
            "Node bounds are invalid.",
            "Use finite x, y, width, and height with positive width and height.",
        ));
    }
}
