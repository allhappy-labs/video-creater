use video_creater_lib::gpu_graphics::error::GpuGraphicsErrorCode;
use video_creater_lib::gpu_graphics::ir::{GpuGraphicRole, GpuGraphicsLayer};
use video_creater_lib::gpu_graphics::mesh::{build_cube_mesh, build_grid_mesh};
use video_creater_lib::gpu_graphics::primitive_renderer::{
    build_wireframe_edges_for_primitive, draw_projected_wireframes_into_rgba,
    project_wireframe_edges,
};
use video_creater_lib::gpu_graphics::profile::{
    collected_shadertoy_profile_ids, expand_profile, profile_id_name, supported_profile_ids,
    supports_cpu_fallback, GpuProfileExpansionInput, GpuVisualProfileId,
};
use video_creater_lib::gpu_graphics::renderer::{render_gpu_graphics_layer, GpuRenderOptions};
use video_creater_lib::gpu_graphics::shader::{
    validate_fragment_source, wrap_glsl_fragment_source,
};
use video_creater_lib::gpu_graphics::software_renderer::{
    render_hq_profile_software, SoftwareGpuRenderOptions,
};
use video_creater_lib::gpu_graphics::templates::{
    builtin_shader_background_templates, load_user_shader_background_templates,
    ShaderTemplateSourceKind,
};
use video_creater_lib::gpu_graphics::validation::{
    validate_gpu_graphics_layer, MAX_SHADER_SOURCE_BYTES,
};
use video_creater_lib::gpu_graphics::visual_qa::{run_hq_visual_qa, VisualQaOptions};
use video_creater_lib::graphics::ir::Dimensions;
use video_creater_lib::graphics::manifest::GraphicsArtifactManifest;

#[test]
fn gpu_shader_background_ir_deserializes_and_validates() {
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");

    validate_gpu_graphics_layer(&layer).expect("minimal shader layer should validate");

    assert_eq!(layer.schema_version, 1);
    assert_eq!(layer.role, GpuGraphicRole::ShaderBackground);
    assert_eq!(
        layer.dimensions,
        Dimensions {
            width: 320,
            height: 180
        }
    );
    assert!(layer.background.is_some());
    assert!(layer.scene.is_none());
}

#[test]
fn gpu_hybrid_scene_ir_deserializes_and_validates() {
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_hybrid_scene_json()).expect("hybrid gpu layer json");

    validate_gpu_graphics_layer(&layer).expect("hybrid layer should validate");

    assert_eq!(layer.role, GpuGraphicRole::HybridScene);
    assert_eq!(
        layer
            .scene
            .as_ref()
            .expect("scene")
            .primitives
            .first()
            .expect("primitive")
            .primitive_type
            .as_str(),
        "cube"
    );
}

#[test]
fn hq_profile_shader_uses_dynamic_render_aspect_ratio() {
    let layer = expand_profile(
        GpuVisualProfileId::HqNeonWireframeShaderV1,
        GpuProfileExpansionInput {
            id: "hq-profile".to_string(),
            timeline_start: 0.0,
            duration_seconds: 1.0,
            dimensions: Dimensions {
                width: 1080,
                height: 1920,
            },
            fps: 30.0,
            source_beat: "portrait generated hook".to_string(),
        },
    );
    let fragment_source = &layer
        .background
        .as_ref()
        .expect("hq profile background")
        .fragment_source;

    assert!(fragment_source.contains("u_resolution.x"));
    assert!(fragment_source.contains("max(u_resolution.y, 1.0)"));
    assert!(!fragment_source.contains("1.7777778"));
}

#[test]
fn hq_profile_declares_cpu_fallback_support() {
    assert!(supports_cpu_fallback(
        GpuVisualProfileId::HqNeonWireframeShaderV1
    ));
}

#[test]
fn collected_shadertoy_profiles_are_all_registered_and_expandable() {
    assert_eq!(collected_shadertoy_profile_ids().len(), 17);
    assert!(supported_profile_ids().contains(&"hq-neon-wireframe-shader-v1"));

    for profile_id in collected_shadertoy_profile_ids().iter().copied() {
        let parsed = video_creater_lib::gpu_graphics::profile::parse_profile_id(profile_id)
            .unwrap_or_else(|| panic!("{profile_id} should parse"));
        assert_eq!(profile_id_name(parsed), profile_id);
        assert!(
            !supports_cpu_fallback(parsed),
            "{profile_id} should not claim the HQ software fallback"
        );

        let layer = expand_profile(
            parsed,
            GpuProfileExpansionInput {
                id: format!("layer-{profile_id}"),
                timeline_start: 0.0,
                duration_seconds: 1.0,
                dimensions: Dimensions {
                    width: 320,
                    height: 180,
                },
                fps: 4.0,
                source_beat: format!("Render {profile_id} as a generated background."),
            },
        );

        assert_eq!(layer.role, GpuGraphicRole::ShaderBackground);
        assert!(!layer.alpha);
        assert_eq!(layer.quality_profile.as_deref(), Some(profile_id));
        assert!(layer.background.is_some());
        assert!(layer.scene.is_none());
        assert!(layer.visual_treatment.len() > 20);
        assert!(layer.motion.len() > 20);
        assert!(layer.safe_zone.contains("10%"));
        assert!(layer.avoid.contains("strobing"));

        let source = &layer
            .background
            .as_ref()
            .expect("shader background")
            .fragment_source;
        assert!(source.contains("video_creater_fragment"));
        assert!(source.contains("vc_noise") || source.contains("vc_hash"));
        assert!(!source.contains("void mainImage"));
        assert!(!source.contains("shadertoy.com"));
        validate_gpu_graphics_layer(&layer).expect("profile layer should validate");
        wrap_glsl_fragment_source(source).expect("profile source should satisfy wrapper contract");
    }
}

#[test]
fn collected_shadertoy_profiles_are_backed_by_template_folders_and_configs() {
    let templates =
        builtin_shader_background_templates().expect("built-in shader template configs load");
    let collected_ids = collected_shadertoy_profile_ids();

    assert_eq!(templates.len(), collected_ids.len());

    for profile_id in collected_ids.iter().copied() {
        let template = templates
            .iter()
            .find(|template| template.config.id == profile_id)
            .unwrap_or_else(|| panic!("{profile_id} should have a template folder"));

        assert_eq!(
            template.config.source_kind,
            ShaderTemplateSourceKind::BuiltIn
        );
        assert_eq!(template.config.shader_profile_id, profile_id);
        assert_eq!(template.config.render_contract.dimensions, "project");
        assert_eq!(template.config.render_contract.fps, "project");
        assert_eq!(template.config.placement.track_kind, "hyperframe_scene");
        assert!(template
            .config_path
            .contains(&format!("builtin/{profile_id}/template.json")));
        assert!(template
            .shader_path
            .contains(&format!("builtin/{profile_id}/shader.frag")));
        assert!(template
            .config
            .utility_refs
            .contains(&"shared/utils.glsl".to_string()));
        assert!(template.config.agent_summary.contains(profile_id));
        assert!(template.config.visual_treatment.len() > 20);
        assert!(template.config.motion.len() > 20);
        assert!(template.config.safe_zone.contains("10%"));
        assert!(template.config.avoid.contains("strobing"));
        assert!(template.fragment_body.contains("video_creater_fragment"));
        assert!(!template.fragment_body.contains("void mainImage"));
    }
}

#[test]
fn shader_wrapper_accepts_shadertoy_main_image_sources() {
    let source = r#"
precision highp float;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 p = (fragCoord * 2.0 - iResolution.xy) / min(iResolution.x, iResolution.y);
    fragColor = vec4(0.5 + 0.5 * cos(iTime + p.xyx + vec3(0.0, 2.0, 4.0)), 1.0);
}
"#;

    let wrapped = wrap_glsl_fragment_source(source).expect("ShaderToy mainImage should wrap");

    assert!(wrapped.contains("void mainImage"));
    assert!(wrapped.contains("vec3 iResolution"));
    assert!(wrapped.contains("float iTime"));
    assert!(wrapped.contains("mainImage(o_color, v_uv * u_resolution)"));
    assert!(wrapped.contains("o_color.a = 1.0;"));
}

#[test]
fn shader_wrapper_accepts_shadertoy_channel_sources() {
    let source = r#"
vec4 sampleChannel(sampler2D tex, vec2 uv) {
    return texture(tex, uv) + texelFetch(tex, ivec2(uv * 32.0), 0) * 0.1;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    fragColor = sampleChannel(iChannel0, uv) + textureLod(iChannel2, uv, 0.0) * 0.1;
}
"#;

    let wrapped = wrap_glsl_fragment_source(source).expect("ShaderToy iChannel source should wrap");

    assert!(wrapped.contains("struct VcChannel"));
    assert!(wrapped.contains("vec4 texture(VcChannel channel, vec2 uv)"));
    assert!(!wrapped.contains("sampler2D"));
    assert!(wrapped.contains("VcChannel iChannel0"));
    assert!(wrapped.contains("VcChannel iChannel2"));
}

#[test]
fn shader_wrapper_selects_buffer_pass_from_multipass_shadertoy_capture() {
    let source = r#"
# image
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = texture(iChannel0, fragCoord / iResolution.xy);
}

# buffer A
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(0.0, 1.0, 0.0, 1.0);
}
"#;

    let wrapped =
        wrap_glsl_fragment_source(source).expect("ShaderToy multipass capture should wrap");

    assert!(!wrapped.contains("# image"));
    assert!(!wrapped.contains("# buffer A"));
    assert!(!wrapped.contains("texture(iChannel0"));
    assert!(wrapped.contains("vec4(0.0, 1.0, 0.0, 1.0)"));
}

#[test]
fn shader_source_budget_fits_original_template_library() {
    const {
        assert!(
            MAX_SHADER_SOURCE_BYTES >= 64 * 1024,
            "shader template sources and shared procedural utilities need room for high-detail variants"
        );
    }
}

#[test]
fn builtin_shadertoy_templates_use_original_video_creater_variants() {
    let templates =
        builtin_shader_background_templates().expect("built-in shader template configs load");

    for template in templates {
        assert!(
            template.fragment_body.contains("video_creater_fragment"),
            "{} should expose the native Video Creater fragment entry",
            template.config.id
        );
        assert!(
            !template.fragment_body.contains("void mainImage"),
            "{} should not be a direct ShaderToy mainImage source port",
            template.config.id
        );
        assert!(
            !template.fragment_body.contains("shadertoy.com"),
            "{} should not carry ShaderToy source URLs in the GLSL body",
            template.config.id
        );
    }
}

#[test]
fn user_shader_background_templates_load_from_project_folders() {
    let root = tempfile::tempdir().expect("template root");
    let template_dir = root.path().join("user-neon-v1");
    std::fs::create_dir_all(&template_dir).expect("template dir");
    std::fs::write(
        template_dir.join("template.json"),
        r##"{
  "id": "user-neon-v1",
  "version": 1,
  "title": "User Neon",
  "sourceKind": "user",
  "category": "user",
  "shaderProfileId": "user-neon-v1",
  "defaultDurationSeconds": 3.0,
  "sourceUrl": null,
  "license": "user-provided",
  "shaderRef": "shader.frag",
  "utilityRefs": ["shared/utils.glsl"],
  "placement": { "trackKind": "hyperframe_scene", "defaultStartSeconds": 0.0 },
  "renderContract": { "dimensions": "project", "fps": "project", "alpha": false },
  "preview": {
    "thumbnailKind": "css",
    "accentColor": "#38bdf8",
    "secondaryColor": "#f97316",
    "description": "User-authored neon field"
  },
  "visualTreatment": "user-authored shader background with neon gradients and soft depth",
  "motion": "slow procedural shimmer with stable low-frequency drift",
  "safeZone": "keep essential motion inside 10% margins",
  "avoid": "strobing, external textures, URLs, and unsafe high-frequency noise",
  "agentSummary": "user-neon-v1: User Neon. kind shader_background.",
  "notes": ["Loaded from a project-local user template folder."]
}"##,
    )
    .expect("template config");
    std::fs::write(
        template_dir.join("shader.frag"),
        "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, 0.5 + 0.5 * sin(time), 1.0); }",
    )
    .expect("template shader");

    let templates =
        load_user_shader_background_templates(root.path()).expect("user shader templates load");

    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].config.id, "user-neon-v1");
    assert_eq!(
        templates[0].config.source_kind,
        ShaderTemplateSourceKind::User
    );
    assert_eq!(templates[0].fragment_body.trim_start()[..4], *"vec4");
}

#[test]
fn collected_shadertoy_profiles_render_nonblank_frames_when_gpu_available() {
    for profile_id in collected_shadertoy_profile_ids().iter().copied() {
        let parsed = video_creater_lib::gpu_graphics::profile::parse_profile_id(profile_id)
            .unwrap_or_else(|| panic!("{profile_id} should parse"));
        let layer = expand_profile(
            parsed,
            GpuProfileExpansionInput {
                id: format!("render-{profile_id}"),
                timeline_start: 0.0,
                duration_seconds: 0.25,
                dimensions: Dimensions {
                    width: 96,
                    height: 54,
                },
                fps: 1.0,
                source_beat: format!("Render smoke test for {profile_id}."),
            },
        );
        let dir = tempfile::tempdir().expect("temp shader render dir");

        let result = render_gpu_graphics_layer(
            &layer,
            GpuRenderOptions {
                output_dir: dir.path().join(profile_id),
            },
        );

        let manifest = match result {
            Ok(manifest) => manifest,
            Err(errors)
                if errors.iter().any(|error| {
                    error.code == GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable
                }) =>
            {
                eprintln!("Skipping collected shader render assertions because no compatible adapter is available");
                return;
            }
            Err(errors) => panic!("{profile_id} failed to render: {errors:?}"),
        };

        assert_eq!(manifest.frame_count, 1, "{profile_id}");
        let frame = image::open(dir.path().join(profile_id).join("frames/frame-000000.png"))
            .unwrap_or_else(|error| panic!("{profile_id} frame should be readable: {error}"))
            .to_rgba8();
        let visible_pixels = frame
            .pixels()
            .filter(|pixel| pixel[3] > 0 && (pixel[0] > 3 || pixel[1] > 3 || pixel[2] > 3))
            .count();
        assert!(
            visible_pixels > 64,
            "{profile_id} should render a nonblank frame"
        );
    }
}

#[test]
fn gpu_validation_rejects_missing_visual_metadata_actionably() {
    let mut layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");
    layer.motion.clear();

    let errors = validate_gpu_graphics_layer(&layer).expect_err("motion is required");

    assert_eq!(errors[0].code, GpuGraphicsErrorCode::GpuGraphicsTextEmpty);
    assert_eq!(errors[0].path, "motion");
    assert!(errors[0].fix.contains("motion"));
}

#[test]
fn gpu_validation_rejects_oversized_frame_budget() {
    let mut layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");
    layer.duration_seconds = 20.5;
    layer.fps = 60.0;

    let errors = validate_gpu_graphics_layer(&layer).expect_err("frame budget should fail");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsFrameBudgetExceeded
    );
    assert_eq!(errors[0].path, "frames");
    assert_eq!(
        errors[0].details.get("maxFrameCount"),
        Some(&"600".to_string())
    );
}

#[test]
fn shader_wrapper_injects_fixed_uniforms_and_entrypoint() {
    let source = "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, time, 1.0); }";

    let wrapped = wrap_glsl_fragment_source(source).expect("valid source should wrap");

    assert!(wrapped.contains("#version 450"));
    assert!(wrapped.contains("layout(location = 0) in vec2 v_uv;"));
    assert!(wrapped.contains("layout(location = 0) out vec4 o_color;"));
    assert!(wrapped.contains("uniform VideoCreaterUniforms"));
    assert!(wrapped.contains("o_color = video_creater_fragment(v_uv, u_time, u_progress);"));
}

#[test]
fn shader_validation_rejects_missing_contract_function() {
    let errors = validate_fragment_source("void main() {}").expect_err("missing contract");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsShaderEntryMissing
    );
    assert_eq!(errors[0].path, "fragmentSource");
}

#[test]
fn shader_validation_rejects_disallowed_preprocessor_and_external_resources() {
    for source in [
        "#include \"private.glsl\"\nvec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, time, 1.0); }",
        "#pragma optimize(on)\nvec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, time, 1.0); }",
        "uniform sampler2D remoteTexture;\nvec4 video_creater_fragment(vec2 uv, float time, float progress) { return texture(remoteTexture, uv); }",
    ] {
        let errors = validate_fragment_source(source).expect_err("source should be rejected");

        assert_eq!(
            errors[0].code,
            GpuGraphicsErrorCode::GpuGraphicsShaderUnsupportedFeature
        );
        assert!(
            errors[0].fix.contains("bounded GLSL fragment function")
                || errors[0].fix.contains("ShaderToy iChannel compatibility")
        );
    }
}

#[test]
fn shader_validation_allows_attribution_urls_inside_comments() {
    let source = r#"
// Ported from https://www.shadertoy.com/view/example
vec4 video_creater_fragment(vec2 uv, float time, float progress) {
    return vec4(uv, 0.5 + 0.5 * sin(time), 1.0);
}
"#;

    validate_fragment_source(source).expect("comment-only attribution URLs should validate");
}

#[test]
fn cube_mesh_has_finite_vertices_and_indices() {
    let mesh = build_cube_mesh(1.0).expect("cube mesh");

    assert_eq!(mesh.vertices.len(), 24);
    assert_eq!(mesh.indices.len(), 36);
    assert!(mesh
        .vertices
        .iter()
        .flat_map(|vertex| vertex.position)
        .all(f32::is_finite));
    assert!(mesh
        .indices
        .iter()
        .all(|index| (*index as usize) < mesh.vertices.len()));
}

#[test]
fn grid_mesh_has_expected_line_vertices() {
    let mesh = build_grid_mesh(4, 2.0).expect("grid mesh");

    assert_eq!(mesh.indices.len() % 2, 0);
    assert!(mesh.vertices.len() >= 20);
    assert!(mesh
        .vertices
        .iter()
        .flat_map(|vertex| vertex.position)
        .all(f32::is_finite));
}

#[test]
fn mesh_generation_rejects_invalid_inputs_actionably() {
    let cube_errors = build_cube_mesh(0.0).expect_err("zero cube size should fail");
    assert_eq!(
        cube_errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );

    let grid_errors = build_grid_mesh(0, 2.0).expect_err("zero grid divisions should fail");
    assert_eq!(
        grid_errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );
}

#[test]
fn primitive_renderer_builds_cube_wireframe_edges() {
    let layer = minimal_hybrid_scene_layer();
    let primitive = layer
        .scene
        .as_ref()
        .expect("scene")
        .primitives
        .first()
        .expect("cube primitive");

    let edges = build_wireframe_edges_for_primitive(primitive).expect("cube wireframe edges");

    assert!(edges.len() >= 12);
    assert!(edges.iter().all(|edge| edge
        .start
        .iter()
        .chain(edge.end.iter())
        .all(|coord| coord.is_finite())));
}

#[test]
fn primitive_renderer_projects_edges_into_screen_space() {
    let layer = minimal_hybrid_scene_layer();
    let primitive = layer
        .scene
        .as_ref()
        .expect("scene")
        .primitives
        .first()
        .expect("cube primitive");
    let edges = build_wireframe_edges_for_primitive(primitive).expect("cube wireframe edges");

    let projected = project_wireframe_edges(&edges, 320, 180, 0.25);

    assert!(!projected.is_empty());
    assert!(projected.iter().all(|edge| edge
        .start
        .iter()
        .chain(edge.end.iter())
        .all(|coord| coord.is_finite())));
}

#[test]
fn primitive_renderer_prefixes_compositor_errors_without_internal_primitive_segment() {
    let mut layer = minimal_hybrid_scene_layer();
    let scene = layer.scene.as_mut().expect("scene");
    scene.primitives[0].transform.scale = [0.0, 1.0, 1.0];
    let mut rgba = vec![0; 64 * 36 * 4];

    let errors =
        draw_projected_wireframes_into_rgba(&mut rgba, 64, 36, scene, 0.25).expect_err("bad scale");

    assert_eq!(errors[0].path, "scene.primitives[0].transform.scale");
}

#[test]
fn primitive_renderer_honors_fixed_camera_position() {
    let mut centered_layer = minimal_hybrid_scene_layer();
    {
        let centered_scene = centered_layer.scene.as_mut().expect("centered scene");
        centered_scene.camera.preset = video_creater_lib::gpu_graphics::ir::CameraPreset::Fixed;
        centered_scene.camera.position = Some([0.0, 0.0, 4.0]);
        centered_scene.camera.target = Some([0.0, 0.0, 0.0]);
        centered_scene.camera.distance = Some(4.0);
    }

    let mut offset_layer = centered_layer.clone();
    offset_layer
        .scene
        .as_mut()
        .expect("offset scene")
        .camera
        .position = Some([1.0, 0.0, 4.0]);

    let mut centered = vec![0; 96 * 54 * 4];
    let mut offset = vec![0; 96 * 54 * 4];
    draw_projected_wireframes_into_rgba(
        &mut centered,
        96,
        54,
        centered_layer.scene.as_ref().expect("centered scene"),
        0.25,
    )
    .expect("centered render");
    draw_projected_wireframes_into_rgba(
        &mut offset,
        96,
        54,
        offset_layer.scene.as_ref().expect("offset scene"),
        0.25,
    )
    .expect("offset render");

    assert_ne!(
        centered, offset,
        "changing fixed camera.position should change primitive projection"
    );
}

#[test]
fn gpu_renderer_writes_shader_frame_sequence_or_reports_unavailable() {
    let dir = tempfile::tempdir().expect("temp gpu render dir");
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");

    let result = render_gpu_graphics_layer(
        &layer,
        GpuRenderOptions {
            output_dir: dir.path().join("gpu-shader"),
        },
    );

    match result {
        Ok(manifest) => {
            assert_eq!(manifest.kind, "rgbaFrameSequence");
            assert_eq!(manifest.frame_count, 4);
            assert_eq!(manifest.duration_seconds, 1.0);
            assert_eq!(manifest.fps, 4.0);
            assert_eq!(manifest.frames_pattern, "frames/frame-%06d.png");
            assert!(dir.path().join("gpu-shader/preview.png").exists());
            assert!(dir.path().join("gpu-shader/manifest.json").exists());
            assert!(dir
                .path()
                .join("gpu-shader/frames/frame-000000.png")
                .exists());
            assert!(dir
                .path()
                .join("gpu-shader/frames/frame-000003.png")
                .exists());
        }
        Err(errors)
            if errors
                .iter()
                .any(|error| error.code == GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable) =>
        {
            eprintln!("Skipping GPU render assertion because no compatible adapter is available");
        }
        Err(errors) => panic!("unexpected GPU renderer errors: {errors:?}"),
    }
}

#[test]
fn software_renderer_writes_hq_profile_manifest_and_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = hq_profile_layer(0.25);

    let manifest = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect("software render should succeed");

    assert_eq!(manifest.kind, "rgbaFrameSequence");
    assert_eq!(manifest.frame_count, 1);
    assert!(dir.path().join("manifest.json").exists());
    assert!(dir.path().join("preview.png").exists());
    assert!(dir.path().join("frames/frame-000000.png").exists());

    let written_manifest: GraphicsArtifactManifest =
        serde_json::from_slice(&std::fs::read(dir.path().join("manifest.json")).expect("manifest"))
            .expect("manifest json");
    assert_eq!(written_manifest, manifest);
}

#[test]
fn software_renderer_hq_profile_frames_are_nonblank() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = hq_profile_layer(1.0);

    render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect("software render should succeed");

    let first = image::open(dir.path().join("frames/frame-000000.png"))
        .expect("first frame")
        .to_rgba8();
    let last = image::open(dir.path().join("frames/frame-000003.png"))
        .expect("last frame")
        .to_rgba8();
    assert!(first
        .pixels()
        .any(|pixel| pixel[3] > 0 && (pixel[0] > 20 || pixel[1] > 20 || pixel[2] > 20)));
    assert_ne!(
        first.as_raw(),
        last.as_raw(),
        "software fallback should animate over time"
    );
}

#[test]
fn software_renderer_hq_profile_frames_are_repeatable() {
    let first_dir = tempfile::tempdir().expect("first temp dir");
    let second_dir = tempfile::tempdir().expect("second temp dir");
    let layer = hq_profile_layer(1.0);

    render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: first_dir.path().to_path_buf(),
        },
    )
    .expect("first software render should succeed");
    render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: second_dir.path().to_path_buf(),
        },
    )
    .expect("second software render should succeed");

    for frame_name in ["frame-000000.png", "frame-000003.png"] {
        let first = std::fs::read(first_dir.path().join("frames").join(frame_name))
            .expect("first frame bytes");
        let second = std::fs::read(second_dir.path().join("frames").join(frame_name))
            .expect("second frame bytes");
        assert_eq!(first, second, "{frame_name} should be deterministic");
    }
}

#[test]
fn visual_qa_rejects_blank_hq_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("frames")).expect("frames dir");
    for index in 0..3 {
        image::RgbaImage::from_pixel(120, 68, image::Rgba([0, 0, 0, 0]))
            .save(dir.path().join(format!("frames/frame-{index:06}.png")))
            .expect("write blank frame");
    }

    let errors = run_hq_visual_qa(dir.path(), 120, 68, 3, VisualQaOptions::default())
        .expect_err("blank frames should fail QA");

    assert_eq!(errors[0].path, "visualQa.samples");
    assert!(errors[0].message.contains("blank"));
}

#[test]
fn visual_qa_rejects_saturated_blob_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("frames")).expect("frames dir");
    for index in 0..3 {
        image::RgbaImage::from_pixel(120, 68, image::Rgba([255, 0, 220, 255]))
            .save(dir.path().join(format!("frames/frame-{index:06}.png")))
            .expect("write blob frame");
    }

    let errors = run_hq_visual_qa(dir.path(), 120, 68, 3, VisualQaOptions::default())
        .expect_err("blob frames should fail QA");

    assert_eq!(errors[0].path, "visualQa.blobDominance");
}

#[test]
fn visual_qa_rejects_single_blank_sample_even_when_aggregate_is_visible() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("frames")).expect("frames dir");
    write_edge_rich_test_frame(dir.path().join("frames/frame-000000.png"), 0);
    image::RgbaImage::from_pixel(120, 68, image::Rgba([0, 0, 0, 0]))
        .save(dir.path().join("frames/frame-000001.png"))
        .expect("write blank frame");
    write_edge_rich_test_frame(dir.path().join("frames/frame-000002.png"), 1);

    let errors = run_hq_visual_qa(dir.path(), 120, 68, 3, VisualQaOptions::default())
        .expect_err("one blank sample should fail QA");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsVisualQaFailed
    );
    assert_eq!(errors[0].path, "visualQa.samples");
    assert_eq!(
        errors[0].details.get("sampleFrame"),
        Some(&"frames/frame-000001.png".to_string())
    );
}

#[test]
fn visual_qa_rejects_single_saturated_blob_sample_even_when_aggregate_has_edges() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(dir.path().join("frames")).expect("frames dir");
    write_edge_rich_test_frame(dir.path().join("frames/frame-000000.png"), 0);
    image::RgbaImage::from_pixel(120, 68, image::Rgba([255, 0, 220, 255]))
        .save(dir.path().join("frames/frame-000001.png"))
        .expect("write saturated frame");
    write_edge_rich_test_frame(dir.path().join("frames/frame-000002.png"), 1);

    let errors = run_hq_visual_qa(dir.path(), 120, 68, 3, VisualQaOptions::default())
        .expect_err("one saturated blob sample should fail QA");

    assert_eq!(errors[0].path, "visualQa.blobDominance");
    assert_eq!(
        errors[0].details.get("sampleFrame"),
        Some(&"frames/frame-000001.png".to_string())
    );
}

#[test]
fn visual_qa_accepts_software_hq_profile_frames() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = hq_profile_layer(0.75);
    let manifest = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect("software render should succeed");

    let report = run_hq_visual_qa(
        dir.path(),
        manifest.dimensions.width,
        manifest.dimensions.height,
        manifest.frame_count,
        VisualQaOptions::default(),
    )
    .expect("software HQ fallback should pass visual QA");

    assert_eq!(report.sampled_frames.len(), 3);
    assert!(report.opaque_ratio > 0.95);
    assert!(report.edge_ratio > 0.01);
    assert!(report.temporal_delta > 0.0);
}

#[test]
fn visual_qa_accepts_one_frame_software_hq_profile_without_motion_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = hq_profile_layer(0.25);
    let manifest = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect("software render should succeed");
    assert_eq!(manifest.frame_count, 1);

    let report = run_hq_visual_qa(
        dir.path(),
        manifest.dimensions.width,
        manifest.dimensions.height,
        manifest.frame_count,
        VisualQaOptions::default(),
    )
    .expect("one-frame HQ fallback should not fail motion QA");

    assert_eq!(
        report.sampled_frames,
        vec!["frames/frame-000000.png".to_string()]
    );
}

#[test]
fn software_renderer_rejects_non_hq_profile_layer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_shader_background_json()).expect("gpu layer json");

    let errors = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect_err("non-HQ layer should be rejected");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );
    assert_eq!(errors[0].path, "profile");
    assert!(errors[0].fix.contains("hq-neon-wireframe-shader-v1"));
}

#[test]
fn software_renderer_rejects_noncanonical_hq_profile_layer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut layer = hq_profile_layer(0.25);
    layer
        .scene
        .as_mut()
        .expect("hq scene")
        .primitives
        .first_mut()
        .expect("first primitive")
        .id = "impostor-wire-cube".to_string();

    let errors = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect_err("noncanonical HQ-looking layer should be rejected");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );
    assert_eq!(errors[0].path, "profile");
    assert!(errors[0].fix.contains("hq-neon-wireframe-shader-v1"));
}

#[test]
fn software_renderer_rejects_hq_profile_layers_with_canonical_field_mutations() {
    assert_software_renderer_rejects_mutated_hq_profile_layer(|layer| {
        layer.scene.as_mut().expect("hq scene").primitives[0].primitive_type = "grid".to_string();
    });
    assert_software_renderer_rejects_mutated_hq_profile_layer(|layer| {
        layer.scene.as_mut().expect("hq scene").primitives[0]
            .material
            .color = "#ffffff".to_string();
    });
    assert_software_renderer_rejects_mutated_hq_profile_layer(|layer| {
        layer.scene.as_mut().expect("hq scene").primitives[0]
            .transform
            .scale = [1.0, 0.92, 0.92];
    });
    assert_software_renderer_rejects_mutated_hq_profile_layer(|layer| {
        layer.scene.as_mut().expect("hq scene").camera.distance = Some(5.0);
    });
    assert_software_renderer_rejects_mutated_hq_profile_layer(|layer| {
        layer.alpha = false;
    });
}

#[test]
fn software_renderer_removes_stale_top_level_artifacts_before_partial_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layer = hq_profile_layer(0.25);
    std::fs::write(dir.path().join("manifest.json"), "stale manifest").expect("stale manifest");
    std::fs::write(dir.path().join("preview.png"), "stale preview").expect("stale preview");
    std::fs::write(dir.path().join("frames"), "not a directory").expect("frames path conflict");

    let errors = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect_err("frames path conflict should fail render");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsArtifactWriteFailed
    );
    assert!(!dir.path().join("manifest.json").exists());
    assert!(!dir.path().join("preview.png").exists());
}

#[test]
fn gpu_renderer_writes_hybrid_scene_manifest_or_reports_unavailable() {
    let dir = tempfile::tempdir().expect("temp gpu render dir");
    let layer: GpuGraphicsLayer =
        serde_json::from_value(minimal_hybrid_scene_json()).expect("hybrid layer json");

    let result = render_gpu_graphics_layer(
        &layer,
        GpuRenderOptions {
            output_dir: dir.path().join("gpu-hybrid"),
        },
    );

    match result {
        Ok(manifest) => {
            assert_eq!(manifest.source_layer_ids, vec!["gpu-hybrid-scene"]);
            assert_eq!(manifest.frame_count, 4);
            assert!(dir
                .path()
                .join("gpu-hybrid/frames/frame-000002.png")
                .exists());
            let first_frame = image::open(dir.path().join("gpu-hybrid/frames/frame-000000.png"))
                .expect("first hybrid frame")
                .to_rgba8();
            let primitive_bright_pixels = first_frame
                .pixels()
                .filter(|pixel| pixel[3] > 0 && pixel[1] > 160 && pixel[2] > 120)
                .count();
            assert!(
                primitive_bright_pixels > 12,
                "expected primitive wireframe pixels composited into the hybrid GPU frame"
            );
        }
        Err(errors)
            if errors
                .iter()
                .any(|error| error.code == GpuGraphicsErrorCode::GpuGraphicsDeviceUnavailable) =>
        {
            eprintln!(
                "Skipping hybrid GPU render assertion because no compatible adapter is available"
            );
        }
        Err(errors) => panic!("unexpected GPU renderer errors: {errors:?}"),
    }
}

fn hq_profile_layer(duration_seconds: f64) -> GpuGraphicsLayer {
    expand_profile(
        GpuVisualProfileId::HqNeonWireframeShaderV1,
        GpuProfileExpansionInput {
            id: "hq-profile".to_string(),
            timeline_start: 0.0,
            duration_seconds,
            dimensions: video_creater_lib::graphics::ir::Dimensions {
                width: 160,
                height: 90,
            },
            fps: 4.0,
            source_beat: "test hq fallback".to_string(),
        },
    )
}

fn minimal_hybrid_scene_layer() -> GpuGraphicsLayer {
    serde_json::from_value(minimal_hybrid_scene_json()).expect("hybrid layer json")
}

fn write_edge_rich_test_frame(path: impl AsRef<std::path::Path>, phase: u32) {
    image::RgbaImage::from_fn(120, 68, |x, y| {
        if ((x / 4) + (y / 4) + phase).is_multiple_of(2) {
            image::Rgba([232, 232, 232, 255])
        } else {
            image::Rgba([24, 84, 124, 255])
        }
    })
    .save(path)
    .expect("write edge-rich frame");
}

fn assert_software_renderer_rejects_mutated_hq_profile_layer(
    mutate: impl FnOnce(&mut GpuGraphicsLayer),
) {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut layer = hq_profile_layer(0.25);
    mutate(&mut layer);

    let errors = render_hq_profile_software(
        &layer,
        SoftwareGpuRenderOptions {
            output_dir: dir.path().to_path_buf(),
        },
    )
    .expect_err("mutated HQ layer should be rejected");

    assert_eq!(
        errors[0].code,
        GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid
    );
    assert_eq!(errors[0].path, "profile");
    assert!(errors[0].fix.contains("hq-neon-wireframe-shader-v1"));
}

fn minimal_shader_background_json() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "id": "gpu-shader-bg",
        "role": "shader_background",
        "timelineStart": 0.0,
        "durationSeconds": 1.0,
        "dimensions": { "width": 320, "height": 180 },
        "fps": 4.0,
        "alpha": true,
        "sourceBeat": "Open with a procedural generated background.",
        "visualTreatment": "soft procedural gradient field",
        "motion": "slow shader drift",
        "safeZone": "keep center low contrast",
        "avoid": "strobing and tiny high-frequency noise",
        "background": {
            "shaderLanguage": "glsl",
            "fragmentSource": "vec4 video_creater_fragment(vec2 uv, float time, float progress) { return vec4(uv, progress, 1.0); }",
            "uniforms": { "speed": 0.75, "enabled": true, "paletteA": "#101820" }
        }
    })
}

fn minimal_hybrid_scene_json() -> serde_json::Value {
    let mut value = minimal_shader_background_json();
    value["id"] = serde_json::json!("gpu-hybrid-scene");
    value["role"] = serde_json::json!("hybrid_scene");
    value["scene"] = serde_json::json!({
        "camera": {
            "preset": "orbit",
            "distance": 4.0,
            "fovDegrees": 45.0
        },
        "primitives": [
            {
                "id": "main-cube",
                "type": "cube",
                "transform": {
                    "position": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0],
                    "scale": [1.0, 1.0, 1.0]
                },
                "material": {
                    "kind": "flat",
                    "color": "#63e6be",
                    "opacity": 1.0
                },
                "animate": {
                    "rotation": { "axis": [0.0, 1.0, 0.0], "turns": 1.0 }
                }
            }
        ]
    });
    value
}
