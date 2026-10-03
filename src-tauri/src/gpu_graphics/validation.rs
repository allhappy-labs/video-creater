use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::ir::{GpuGraphicRole, GpuGraphicsLayer, PrimitiveScene, ShaderPass};

const SUPPORTED_SCHEMA_VERSION: u32 = 1;
pub const MAX_SHADER_SOURCE_BYTES: usize = 64 * 1024;
pub const MAX_WIDTH: u32 = 1920;
pub const MAX_HEIGHT: u32 = 1080;
pub const MAX_FPS: f64 = 60.0;
pub const MAX_DURATION_SECONDS: f64 = 20.0;
pub const MAX_FRAME_COUNT: u32 = 600;
pub const MAX_PRIMITIVE_COUNT: usize = 256;
pub const MAX_PARTICLE_COUNT: u32 = 10_000;

pub fn validate_gpu_graphics_layer(layer: &GpuGraphicsLayer) -> GpuGraphicsResult<()> {
    let mut errors = Vec::new();

    if layer.schema_version != SUPPORTED_SCHEMA_VERSION {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsSchemaUnsupported,
            "schemaVersion",
            "GPU graphics schema version is not supported.",
            "Set schemaVersion to 1.",
        ));
    }

    if layer.id.trim().is_empty() {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsTextEmpty,
            "id",
            "GPU graphics layer id is empty.",
            "Set id to a stable non-empty string.",
        ));
    }

    validate_required_text(layer, &mut errors);
    validate_timing_and_dimensions(layer, &mut errors);
    validate_role_shape(layer, &mut errors);

    if let Some(background) = &layer.background {
        validate_shader_pass(background, "background", &mut errors);
    }
    if let Some(scene) = &layer.scene {
        validate_scene(scene, "scene", &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_required_text(layer: &GpuGraphicsLayer, errors: &mut Vec<GpuGraphicsError>) {
    for (field, value) in [
        ("sourceBeat", &layer.source_beat),
        ("visualTreatment", &layer.visual_treatment),
        ("motion", &layer.motion),
        ("safeZone", &layer.safe_zone),
        ("avoid", &layer.avoid),
    ] {
        if value.trim().is_empty() {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsTextEmpty,
                field,
                "Required GPU visual metadata is empty.",
                format!("Set {field} to a concise non-empty visual direction."),
            ));
        }
    }
}

fn validate_timing_and_dimensions(layer: &GpuGraphicsLayer, errors: &mut Vec<GpuGraphicsError>) {
    if layer.dimensions.width == 0
        || layer.dimensions.height == 0
        || layer.dimensions.width.max(layer.dimensions.height) > MAX_WIDTH
        || u64::from(layer.dimensions.width) * u64::from(layer.dimensions.height)
            > u64::from(MAX_WIDTH) * u64::from(MAX_HEIGHT)
    {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidDimensions,
            "dimensions",
            "GPU layer dimensions are outside the v1 budget.",
            "Use at most 2,073,600 pixels with a longest edge of 1920 pixels for GPU-generated layers.",
        ));
    }

    if layer.duration_seconds.is_finite() && layer.fps.is_finite() {
        let frame_count = (layer.duration_seconds * layer.fps).ceil().max(1.0) as u32;
        if frame_count > MAX_FRAME_COUNT {
            errors.push(
                GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsFrameBudgetExceeded,
                    "frames",
                    "GPU layer frame budget is too large.",
                    "Reduce durationSeconds or fps so the layer renders no more than 600 frames.",
                )
                .with_detail("frameCount", frame_count.to_string())
                .with_detail("maxFrameCount", MAX_FRAME_COUNT.to_string()),
            );
        }
    }

    if !layer.timeline_start.is_finite() || layer.timeline_start < 0.0 {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidTiming,
            "timelineStart",
            "GPU layer start time is invalid.",
            "Use a finite timelineStart greater than or equal to 0.",
        ));
    }

    if !layer.duration_seconds.is_finite()
        || layer.duration_seconds <= 0.0
        || layer.duration_seconds > MAX_DURATION_SECONDS
    {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidTiming,
            "durationSeconds",
            "GPU layer duration is outside the v1 budget.",
            "Use a finite durationSeconds greater than 0 and no more than 20.",
        ));
    }

    if !layer.fps.is_finite() || layer.fps <= 0.0 || layer.fps > MAX_FPS {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsInvalidTiming,
            "fps",
            "GPU layer FPS is outside the v1 budget.",
            "Use a finite fps greater than 0 and no more than 60.",
        ));
    }
}

fn validate_role_shape(layer: &GpuGraphicsLayer, errors: &mut Vec<GpuGraphicsError>) {
    match layer.role {
        GpuGraphicRole::ShaderBackground => {
            if layer.background.is_none() || layer.scene.is_some() {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    "role",
                    "shader_background layers require background and no scene.",
                    "Use role hybrid_scene when combining a shader with primitives.",
                ));
            }
        }
        GpuGraphicRole::PrimitiveScene => {
            if layer.scene.is_none() {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    "scene",
                    "primitive_scene layers require a scene.",
                    "Add scene.camera and at least one primitive.",
                ));
            }
        }
        GpuGraphicRole::HybridScene => {
            if layer.background.is_none() || layer.scene.is_none() {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    "role",
                    "hybrid_scene layers require both background and scene.",
                    "Add a background shader and scene primitives.",
                ));
            }
        }
    }
}

fn validate_shader_pass(pass: &ShaderPass, path: &str, errors: &mut Vec<GpuGraphicsError>) {
    if let Err(shader_errors) =
        crate::gpu_graphics::shader::validate_fragment_source(&pass.fragment_source)
    {
        errors.extend(shader_errors.into_iter().map(|mut error| {
            error.path = format!("{path}.{}", error.path);
            error
        }));
    }
}

fn validate_scene(scene: &PrimitiveScene, path: &str, errors: &mut Vec<GpuGraphicsError>) {
    if scene.primitives.is_empty() {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            format!("{path}.primitives"),
            "Primitive scene has no primitives.",
            "Add at least one cube or grid primitive.",
        ));
    }
    if scene.primitives.len() > MAX_PRIMITIVE_COUNT {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            format!("{path}.primitives"),
            "Primitive count exceeds the v1 budget.",
            "Use no more than 256 primitives in one GPU layer.",
        ));
    }

    for (index, primitive) in scene.primitives.iter().enumerate() {
        let primitive_path = format!("{path}.primitives[{index}]");
        if primitive.id.trim().is_empty() {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                format!("{primitive_path}.id"),
                "Primitive id is empty.",
                "Set id to a stable non-empty string.",
            ));
        }
        if primitive.primitive_type != "cube" && primitive.primitive_type != "grid" {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveUnsupported,
                format!("{primitive_path}.type"),
                "Primitive type is not supported in the first GPU slice.",
                "Use cube or grid.",
            ));
        }
        for (field, values) in [
            ("position", primitive.transform.position),
            ("rotation", primitive.transform.rotation),
            ("scale", primitive.transform.scale),
        ] {
            if values.iter().any(|value| !value.is_finite()) {
                errors.push(GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                    format!("{primitive_path}.transform.{field}"),
                    "Primitive transform contains a non-finite value.",
                    "Use finite transform values.",
                ));
            }
        }
        if primitive.transform.scale.iter().any(|value| *value <= 0.0) {
            errors.push(GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
                format!("{primitive_path}.transform.scale"),
                "Primitive scale must be positive.",
                "Use scale values greater than 0.",
            ));
        }
    }
}
