use std::collections::BTreeMap;

use crate::gpu_graphics::ir::{
    Camera, CameraPreset, GpuGraphicRole, GpuGraphicsLayer, Material, MaterialKind, Primitive,
    PrimitiveAnimation, PrimitiveScene, RotationAnimation, ShaderLanguage, ShaderPass, Transform3,
};
use crate::gpu_graphics::templates::{builtin_shader_background_templates, shared_shader_utils};
use crate::graphics::ir::Dimensions;

pub const HQ_NEON_WIREFRAME_SHADER_V1: &str = "hq-neon-wireframe-shader-v1";
pub const SHADERTOY_OCTAGRAMS_V1: &str = "shadertoy-octagrams-v1";
pub const SHADERTOY_BASE_WARP_FBM_V1: &str = "shadertoy-base-warp-fbm-v1";
pub const SHADERTOY_PHANTOM_STAR_V1: &str = "shadertoy-phantom-star-v1";
pub const SHADERTOY_VOLUMETRIC_CLOUDS_V1: &str = "shadertoy-volumetric-clouds-v1";
pub const SHADERTOY_CUBE_LINES_V1: &str = "shadertoy-cube-lines-v1";
pub const SHADERTOY_CRUMPLED_WAVE_V1: &str = "shadertoy-crumpled-wave-v1";
pub const SHADERTOY_GLOWING_MARBLING_BLACK_V1: &str = "shadertoy-glowing-marbling-black-v1";
pub const SHADERTOY_GEODESIC_TILING_V1: &str = "shadertoy-geodesic-tiling-v1";
pub const SHADERTOY_KALEIDOSCOPE_TUNNEL_V1: &str = "shadertoy-kaleidoscope-tunnel-v1";
pub const SHADERTOY_DIGITAL_BRAIN_V1: &str = "shadertoy-digital-brain-v1";
pub const SHADERTOY_STARRY_PLANES_V1: &str = "shadertoy-starry-planes-v1";
pub const SHADERTOY_MANDELBULB_INTERIOR_V1: &str = "shadertoy-mandelbulb-interior-v1";
pub const SHADERTOY_TINY_CLOUDS_V1: &str = "shadertoy-tiny-clouds-v1";
pub const SHADERTOY_NEON_LIT_HEXAGONS_V1: &str = "shadertoy-neon-lit-hexagons-v1";
pub const SHADERTOY_ANOTHER_CUBE_V1: &str = "shadertoy-another-cube-v1";
pub const SHADERTOY_OBJECT_MESH_V1: &str = "shadertoy-object-mesh-v1";
pub const SHADERTOY_LOOPING_CLOUDS_V1: &str = "shadertoy-looping-clouds-v1";

const COLLECTED_SHADERTOY_PROFILE_IDS: &[&str] = &[
    SHADERTOY_OCTAGRAMS_V1,
    SHADERTOY_BASE_WARP_FBM_V1,
    SHADERTOY_PHANTOM_STAR_V1,
    SHADERTOY_VOLUMETRIC_CLOUDS_V1,
    SHADERTOY_CUBE_LINES_V1,
    SHADERTOY_CRUMPLED_WAVE_V1,
    SHADERTOY_GLOWING_MARBLING_BLACK_V1,
    SHADERTOY_GEODESIC_TILING_V1,
    SHADERTOY_KALEIDOSCOPE_TUNNEL_V1,
    SHADERTOY_DIGITAL_BRAIN_V1,
    SHADERTOY_STARRY_PLANES_V1,
    SHADERTOY_MANDELBULB_INTERIOR_V1,
    SHADERTOY_TINY_CLOUDS_V1,
    SHADERTOY_NEON_LIT_HEXAGONS_V1,
    SHADERTOY_ANOTHER_CUBE_V1,
    SHADERTOY_OBJECT_MESH_V1,
    SHADERTOY_LOOPING_CLOUDS_V1,
];

const SUPPORTED_PROFILE_IDS: &[&str] = &[
    HQ_NEON_WIREFRAME_SHADER_V1,
    SHADERTOY_OCTAGRAMS_V1,
    SHADERTOY_BASE_WARP_FBM_V1,
    SHADERTOY_PHANTOM_STAR_V1,
    SHADERTOY_VOLUMETRIC_CLOUDS_V1,
    SHADERTOY_CUBE_LINES_V1,
    SHADERTOY_CRUMPLED_WAVE_V1,
    SHADERTOY_GLOWING_MARBLING_BLACK_V1,
    SHADERTOY_GEODESIC_TILING_V1,
    SHADERTOY_KALEIDOSCOPE_TUNNEL_V1,
    SHADERTOY_DIGITAL_BRAIN_V1,
    SHADERTOY_STARRY_PLANES_V1,
    SHADERTOY_MANDELBULB_INTERIOR_V1,
    SHADERTOY_TINY_CLOUDS_V1,
    SHADERTOY_NEON_LIT_HEXAGONS_V1,
    SHADERTOY_ANOTHER_CUBE_V1,
    SHADERTOY_OBJECT_MESH_V1,
    SHADERTOY_LOOPING_CLOUDS_V1,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVisualProfileId {
    HqNeonWireframeShaderV1,
    ShadertoyOctagramsV1,
    ShadertoyBaseWarpFbmV1,
    ShadertoyPhantomStarV1,
    ShadertoyVolumetricCloudsV1,
    ShadertoyCubeLinesV1,
    ShadertoyCrumpledWaveV1,
    ShadertoyGlowingMarblingBlackV1,
    ShadertoyGeodesicTilingV1,
    ShadertoyKaleidoscopeTunnelV1,
    ShadertoyDigitalBrainV1,
    ShadertoyStarryPlanesV1,
    ShadertoyMandelbulbInteriorV1,
    ShadertoyTinyCloudsV1,
    ShadertoyNeonLitHexagonsV1,
    ShadertoyAnotherCubeV1,
    ShadertoyObjectMeshV1,
    ShadertoyLoopingCloudsV1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuProfileExpansionInput {
    pub id: String,
    pub timeline_start: f64,
    pub duration_seconds: f64,
    pub dimensions: Dimensions,
    pub fps: f64,
    pub source_beat: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadertoyProfileTemplate {
    pub profile_id: String,
    pub title: String,
    pub alpha: bool,
    pub visual_treatment: String,
    pub motion: String,
    pub safe_zone: String,
    pub avoid: String,
    pub fragment_body: String,
}

pub fn supported_profile_ids() -> &'static [&'static str] {
    SUPPORTED_PROFILE_IDS
}

pub fn collected_shadertoy_profile_ids() -> &'static [&'static str] {
    COLLECTED_SHADERTOY_PROFILE_IDS
}

pub fn profile_id_name(profile_id: GpuVisualProfileId) -> &'static str {
    match profile_id {
        GpuVisualProfileId::HqNeonWireframeShaderV1 => HQ_NEON_WIREFRAME_SHADER_V1,
        GpuVisualProfileId::ShadertoyOctagramsV1 => SHADERTOY_OCTAGRAMS_V1,
        GpuVisualProfileId::ShadertoyBaseWarpFbmV1 => SHADERTOY_BASE_WARP_FBM_V1,
        GpuVisualProfileId::ShadertoyPhantomStarV1 => SHADERTOY_PHANTOM_STAR_V1,
        GpuVisualProfileId::ShadertoyVolumetricCloudsV1 => SHADERTOY_VOLUMETRIC_CLOUDS_V1,
        GpuVisualProfileId::ShadertoyCubeLinesV1 => SHADERTOY_CUBE_LINES_V1,
        GpuVisualProfileId::ShadertoyCrumpledWaveV1 => SHADERTOY_CRUMPLED_WAVE_V1,
        GpuVisualProfileId::ShadertoyGlowingMarblingBlackV1 => SHADERTOY_GLOWING_MARBLING_BLACK_V1,
        GpuVisualProfileId::ShadertoyGeodesicTilingV1 => SHADERTOY_GEODESIC_TILING_V1,
        GpuVisualProfileId::ShadertoyKaleidoscopeTunnelV1 => SHADERTOY_KALEIDOSCOPE_TUNNEL_V1,
        GpuVisualProfileId::ShadertoyDigitalBrainV1 => SHADERTOY_DIGITAL_BRAIN_V1,
        GpuVisualProfileId::ShadertoyStarryPlanesV1 => SHADERTOY_STARRY_PLANES_V1,
        GpuVisualProfileId::ShadertoyMandelbulbInteriorV1 => SHADERTOY_MANDELBULB_INTERIOR_V1,
        GpuVisualProfileId::ShadertoyTinyCloudsV1 => SHADERTOY_TINY_CLOUDS_V1,
        GpuVisualProfileId::ShadertoyNeonLitHexagonsV1 => SHADERTOY_NEON_LIT_HEXAGONS_V1,
        GpuVisualProfileId::ShadertoyAnotherCubeV1 => SHADERTOY_ANOTHER_CUBE_V1,
        GpuVisualProfileId::ShadertoyObjectMeshV1 => SHADERTOY_OBJECT_MESH_V1,
        GpuVisualProfileId::ShadertoyLoopingCloudsV1 => SHADERTOY_LOOPING_CLOUDS_V1,
    }
}

pub fn parse_profile_id(profile_id: &str) -> Option<GpuVisualProfileId> {
    match profile_id.trim() {
        HQ_NEON_WIREFRAME_SHADER_V1 => Some(GpuVisualProfileId::HqNeonWireframeShaderV1),
        SHADERTOY_OCTAGRAMS_V1 => Some(GpuVisualProfileId::ShadertoyOctagramsV1),
        SHADERTOY_BASE_WARP_FBM_V1 => Some(GpuVisualProfileId::ShadertoyBaseWarpFbmV1),
        SHADERTOY_PHANTOM_STAR_V1 => Some(GpuVisualProfileId::ShadertoyPhantomStarV1),
        SHADERTOY_VOLUMETRIC_CLOUDS_V1 => Some(GpuVisualProfileId::ShadertoyVolumetricCloudsV1),
        SHADERTOY_CUBE_LINES_V1 => Some(GpuVisualProfileId::ShadertoyCubeLinesV1),
        SHADERTOY_CRUMPLED_WAVE_V1 => Some(GpuVisualProfileId::ShadertoyCrumpledWaveV1),
        SHADERTOY_GLOWING_MARBLING_BLACK_V1 => {
            Some(GpuVisualProfileId::ShadertoyGlowingMarblingBlackV1)
        }
        SHADERTOY_GEODESIC_TILING_V1 => Some(GpuVisualProfileId::ShadertoyGeodesicTilingV1),
        SHADERTOY_KALEIDOSCOPE_TUNNEL_V1 => Some(GpuVisualProfileId::ShadertoyKaleidoscopeTunnelV1),
        SHADERTOY_DIGITAL_BRAIN_V1 => Some(GpuVisualProfileId::ShadertoyDigitalBrainV1),
        SHADERTOY_STARRY_PLANES_V1 => Some(GpuVisualProfileId::ShadertoyStarryPlanesV1),
        SHADERTOY_MANDELBULB_INTERIOR_V1 => Some(GpuVisualProfileId::ShadertoyMandelbulbInteriorV1),
        SHADERTOY_TINY_CLOUDS_V1 => Some(GpuVisualProfileId::ShadertoyTinyCloudsV1),
        SHADERTOY_NEON_LIT_HEXAGONS_V1 => Some(GpuVisualProfileId::ShadertoyNeonLitHexagonsV1),
        SHADERTOY_ANOTHER_CUBE_V1 => Some(GpuVisualProfileId::ShadertoyAnotherCubeV1),
        SHADERTOY_OBJECT_MESH_V1 => Some(GpuVisualProfileId::ShadertoyObjectMeshV1),
        SHADERTOY_LOOPING_CLOUDS_V1 => Some(GpuVisualProfileId::ShadertoyLoopingCloudsV1),
        _ => None,
    }
}

pub fn expand_profile(
    profile_id: GpuVisualProfileId,
    input: GpuProfileExpansionInput,
) -> GpuGraphicsLayer {
    match profile_id {
        GpuVisualProfileId::HqNeonWireframeShaderV1 => expand_hq_neon_wireframe(input),
        _ => expand_shadertoy_shader_background(profile_id, input),
    }
}

pub fn supports_cpu_fallback(profile_id: GpuVisualProfileId) -> bool {
    matches!(profile_id, GpuVisualProfileId::HqNeonWireframeShaderV1)
}

pub fn shadertoy_profile_templates() -> Vec<ShadertoyProfileTemplate> {
    builtin_shader_background_templates()
        .expect("built-in shader background templates should be valid")
        .into_iter()
        .map(|template| ShadertoyProfileTemplate {
            profile_id: template.config.id,
            title: template.config.title,
            alpha: template.config.render_contract.alpha,
            visual_treatment: template.config.visual_treatment,
            motion: template.config.motion,
            safe_zone: template.config.safe_zone,
            avoid: template.config.avoid,
            fragment_body: template.fragment_body,
        })
        .collect()
}

pub fn is_canonical_hq_neon_wireframe_layer(layer: &GpuGraphicsLayer) -> bool {
    let expected = expand_profile(
        GpuVisualProfileId::HqNeonWireframeShaderV1,
        GpuProfileExpansionInput {
            id: layer.id.clone(),
            timeline_start: layer.timeline_start,
            duration_seconds: layer.duration_seconds,
            dimensions: layer.dimensions.clone(),
            fps: layer.fps,
            source_beat: layer.source_beat.clone(),
        },
    );

    layer == &expected
}

fn expand_hq_neon_wireframe(input: GpuProfileExpansionInput) -> GpuGraphicsLayer {
    GpuGraphicsLayer {
        schema_version: 1,
        id: input.id,
        role: GpuGraphicRole::HybridScene,
        timeline_start: input.timeline_start,
        duration_seconds: input.duration_seconds,
        dimensions: input.dimensions,
        fps: input.fps,
        alpha: true,
        source_beat: input.source_beat,
        visual_treatment: "restrained animated gradient shader background with crisp neon wireframe 3D primitives, perspective depth, and subtle motion trails".to_string(),
        motion: "slow shader drift, orbiting wireframe cubes, and a subtle parallax grid push tied to edit progress".to_string(),
        safe_zone: "keep essential contrast and 3D focus inside the central 80%; preserve lower caption and face/action regions".to_string(),
        avoid: "oversized saturated blobs, low-resolution draft looks, filled abstract shapes that do not read as 3D, heavy compression artifacts, strobing, and unsafe high-frequency noise".to_string(),
        quality_profile: Some(HQ_NEON_WIREFRAME_SHADER_V1.to_string()),
        background: Some(ShaderPass {
            shader_language: ShaderLanguage::Glsl,
            fragment_source: hq_neon_wireframe_fragment_source(),
            uniforms: BTreeMap::new(),
        }),
        scene: Some(PrimitiveScene {
            camera: Camera {
                preset: CameraPreset::Orbit,
                position: Some([0.0, 0.35, 4.2]),
                target: Some([0.0, 0.0, 0.0]),
                distance: Some(4.2),
                fov_degrees: Some(46.0),
            },
            primitives: vec![
                Primitive {
                    id: "hero-wire-cube".to_string(),
                    primitive_type: "cube".to_string(),
                    transform: Transform3 {
                        position: [-0.78, 0.08, 0.0],
                        rotation: [0.15, 0.35, 0.0],
                        scale: [0.92, 0.92, 0.92],
                    },
                    material: Material {
                        kind: MaterialKind::Wire,
                        color: "#63e6be".to_string(),
                        opacity: Some(0.82),
                    },
                    animate: Some(PrimitiveAnimation {
                        rotation: Some(RotationAnimation {
                            axis: [0.2, 1.0, 0.1],
                            turns: 0.55,
                        }),
                    }),
                },
                Primitive {
                    id: "accent-wire-cube".to_string(),
                    primitive_type: "cube".to_string(),
                    transform: Transform3 {
                        position: [1.06, -0.16, -0.62],
                        rotation: [-0.08, 0.62, 0.18],
                        scale: [0.44, 0.44, 0.44],
                    },
                    material: Material {
                        kind: MaterialKind::Wire,
                        color: "#ff4fd8".to_string(),
                        opacity: Some(0.74),
                    },
                    animate: Some(PrimitiveAnimation {
                        rotation: Some(RotationAnimation {
                            axis: [0.0, 1.0, 0.35],
                            turns: -0.35,
                        }),
                    }),
                },
                Primitive {
                    id: "perspective-neon-grid".to_string(),
                    primitive_type: "grid".to_string(),
                    transform: Transform3 {
                        position: [0.0, -0.92, -1.18],
                        rotation: [-std::f32::consts::FRAC_PI_2, 0.0, 0.0],
                        scale: [3.8, 3.8, 1.0],
                    },
                    material: Material {
                        kind: MaterialKind::Emissive,
                        color: "#42d9ff".to_string(),
                        opacity: Some(0.48),
                    },
                    animate: Some(PrimitiveAnimation {
                        rotation: Some(RotationAnimation {
                            axis: [0.0, 0.0, 1.0],
                            turns: 0.04,
                        }),
                    }),
                },
            ],
        }),
    }
}

fn expand_shadertoy_shader_background(
    profile_id: GpuVisualProfileId,
    input: GpuProfileExpansionInput,
) -> GpuGraphicsLayer {
    let template =
        shadertoy_template(profile_id).expect("non-HQ profile ids should have Shadertoy templates");
    GpuGraphicsLayer {
        schema_version: 1,
        id: input.id,
        role: GpuGraphicRole::ShaderBackground,
        timeline_start: input.timeline_start,
        duration_seconds: input.duration_seconds,
        dimensions: input.dimensions,
        fps: input.fps,
        alpha: template.alpha,
        source_beat: input.source_beat,
        visual_treatment: template.visual_treatment.to_string(),
        motion: template.motion.to_string(),
        safe_zone: template.safe_zone.to_string(),
        avoid: template.avoid.to_string(),
        quality_profile: Some(template.profile_id.to_string()),
        background: Some(ShaderPass {
            shader_language: ShaderLanguage::Glsl,
            fragment_source: shadertoy_fragment_source(&template.fragment_body),
            uniforms: BTreeMap::new(),
        }),
        scene: None,
    }
}

fn shadertoy_template(profile_id: GpuVisualProfileId) -> Option<ShadertoyProfileTemplate> {
    if profile_id == GpuVisualProfileId::HqNeonWireframeShaderV1 {
        return None;
    }
    let profile_id = profile_id_name(profile_id);
    builtin_shader_background_templates()
        .expect("built-in shader background templates should be valid")
        .into_iter()
        .find(|template| template.config.id == profile_id)
        .map(|template| ShadertoyProfileTemplate {
            profile_id: template.config.id,
            title: template.config.title,
            alpha: template.config.render_contract.alpha,
            visual_treatment: template.config.visual_treatment,
            motion: template.config.motion,
            safe_zone: template.config.safe_zone,
            avoid: template.config.avoid,
            fragment_body: template.fragment_body,
        })
}

fn shadertoy_fragment_source(fragment_body: &str) -> String {
    format!(
        "{}\n\n{}",
        shared_shader_utils().trim(),
        fragment_body.trim()
    )
}

fn hq_neon_wireframe_fragment_source() -> String {
    r#"
vec3 hq_neon_palette(float t) {
    vec3 base = vec3(0.035, 0.055, 0.105);
    vec3 cyan = vec3(0.180, 0.860, 0.960);
    vec3 magenta = vec3(0.980, 0.220, 0.760);
    vec3 amber = vec3(1.000, 0.720, 0.280);
    vec3 wave = 0.5 + 0.5 * cos(6.28318 * (t + vec3(0.00, 0.33, 0.67)));
    return base + cyan * wave.x * 0.34 + magenta * wave.y * 0.20 + amber * wave.z * 0.08;
}

float hq_neon_line(float value, float width) {
    float distance_to_line = abs(fract(value) - 0.5);
    return 1.0 - smoothstep(0.0, width, distance_to_line);
}

vec4 video_creater_fragment(vec2 uv, float time, float progress) {
    vec2 p = uv * 2.0 - 1.0;
    float aspect = u_resolution.x / max(u_resolution.y, 1.0);
    p.x *= aspect;
    float drift = time * 0.035 + progress * 0.42;
    float horizon = smoothstep(-0.42, 0.68, p.y);
    vec3 color = hq_neon_palette(drift + p.x * 0.035 + p.y * 0.052);
    float diagonal = hq_neon_line((p.x * 0.32 + p.y * 0.74 + drift) * 7.0, 0.035);
    float grid = hq_neon_line((p.x + drift * 1.8) * 5.0, 0.026)
        * hq_neon_line((p.y * 1.8 - drift * 1.2) * 5.0, 0.026);
    float vignette = 1.0 - smoothstep(0.22, 1.42, length(p));
    float depth = 0.16 / (0.34 + abs(p.y + 0.20));
    color *= 0.44 + vignette * 0.58;
    color += vec3(0.10, 0.72, 0.94) * diagonal * 0.055;
    color += vec3(0.50, 0.96, 1.00) * grid * depth * horizon * 0.070;
    color += vec3(0.95, 0.22, 0.76) * (1.0 - smoothstep(0.12, 0.96, abs(p.x + sin(time * 0.18) * 0.22))) * 0.030;
    return vec4(color, 1.0);
}
"#
    .trim()
    .to_string()
}
