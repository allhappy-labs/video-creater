use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use super::validation::MAX_SHADER_SOURCE_BYTES;

pub fn validate_fragment_source(source: &str) -> GpuGraphicsResult<()> {
    let mut errors = Vec::new();

    if source.len() > MAX_SHADER_SOURCE_BYTES {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderSourceTooLarge,
            "fragmentSource",
            "GLSL fragment source exceeds the v1 size budget.",
            "Keep fragmentSource at or below the configured ShaderToy compatibility budget.",
        ));
    }

    if !source.contains("video_creater_fragment") && !source.contains("mainImage") {
        errors.push(GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsShaderEntryMissing,
            "fragmentSource",
            "GLSL fragment source is missing a supported fragment entry.",
            "Provide either video_creater_fragment(vec2 uv, float time, float progress) or a ShaderToy mainImage(out vec4 fragColor, in vec2 fragCoord).",
        ));
    }

    let is_shadertoy_source =
        !source.contains("video_creater_fragment") && source.contains("mainImage");
    let raw_source_without_comments = shader_source_without_comments(source);
    if raw_source_without_comments.contains("uniform sampler")
        || (!is_shadertoy_source && raw_source_without_comments.contains("sampler2D"))
    {
        errors.push(
            GpuGraphicsError::new(
                GpuGraphicsErrorCode::GpuGraphicsShaderUnsupportedFeature,
                "fragmentSource",
                "GLSL fragment source uses an unsupported external sampler.",
                "Use ShaderToy iChannel compatibility inputs or procedural functions instead of declaring external sampler uniforms.",
            )
            .with_detail("blockedPattern", "sampler"),
        );
    }

    let prepared_source = prepare_shader_source_for_wrapping(source);
    let source_without_comments = shader_source_without_comments(&prepared_source);
    for blocked in [
        "#include",
        "#pragma",
        "image2D",
        "buffer ",
        "layout(binding",
        "http://",
        "https://",
        "file://",
    ] {
        if source_without_comments.contains(blocked) {
            errors.push(
                GpuGraphicsError::new(
                    GpuGraphicsErrorCode::GpuGraphicsShaderUnsupportedFeature,
                    "fragmentSource",
                    "GLSL fragment source uses an unsupported feature.",
                    "Use one bounded GLSL fragment function without includes, external textures, storage buffers, URLs, or custom bindings.",
                )
                .with_detail("blockedPattern", blocked),
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn wrap_glsl_fragment_source(source: &str) -> GpuGraphicsResult<String> {
    validate_fragment_source(source)?;
    let source = prepare_shader_source_for_wrapping(source);
    let is_shadertoy_source =
        !source.contains("video_creater_fragment") && source.contains("mainImage");
    let shadertoy_globals = if is_shadertoy_source {
        r#"
vec3 iResolution;
float iTime;
float iTimeDelta;
int iFrame;
vec4 iMouse;
vec4 iDate;
float iSampleRate;
"#
    } else {
        ""
    };
    let shadertoy_channels = if is_shadertoy_source && source.contains("iChannel") {
        shadertoy_channel_compatibility_source()
    } else {
        ""
    };
    let fragment_entry = if is_shadertoy_source {
        r#"
    iResolution = vec3(u_resolution, 1.0);
    iTime = u_time;
    iTimeDelta = 1.0 / 60.0;
    iFrame = int(u_frame);
    iMouse = vec4(0.0);
    iDate = vec4(1970.0, 1.0, 1.0, 0.0);
    iSampleRate = 44100.0;
    mainImage(o_color, v_uv * u_resolution);
    o_color.a = 1.0;
"#
    } else {
        r#"
    o_color = video_creater_fragment(v_uv, u_time, u_progress);
"#
    };
    Ok(format!(
        r#"#version 450
layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform VideoCreaterUniforms {{
    float u_time;
    float u_duration;
    vec2 u_resolution;
    float u_frame;
    float u_progress;
}};

{shadertoy_globals}
{shadertoy_channels}
{source}

void main() {{
{fragment_entry}
}}
"#
    ))
}

fn prepare_shader_source_for_wrapping(source: &str) -> String {
    let source = select_single_shadertoy_pass(source);
    let source = shader_source_without_precision_qualifiers(&source);
    source.replace("sampler2D", "VcChannel")
}

fn select_single_shadertoy_pass(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start().to_ascii_lowercase();
            trimmed.starts_with("# buffer").then_some(index + 1)
        })
        .next_back()
        .unwrap_or(0);

    lines[start..]
        .iter()
        .filter(|line| {
            let trimmed = line.trim_start().to_ascii_lowercase();
            !trimmed.starts_with("# image") && !trimmed.starts_with("# buffer")
        })
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

fn shader_source_without_precision_qualifiers(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("precision "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn shader_source_without_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                output.push('\n');
            }
            continue;
        }

        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            } else if ch == '\n' {
                output.push('\n');
            }
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line_comment = true;
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            continue;
        }

        output.push(ch);
    }

    output
}

fn shadertoy_channel_compatibility_source() -> &'static str {
    r#"
struct VcChannel {
    int id;
};

const VcChannel iChannel0 = VcChannel(0);
const VcChannel iChannel1 = VcChannel(1);
const VcChannel iChannel2 = VcChannel(2);
const VcChannel iChannel3 = VcChannel(3);

float vc_channel_hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

float vc_channel_noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = vc_channel_hash(i);
    float b = vc_channel_hash(i + vec2(1.0, 0.0));
    float c = vc_channel_hash(i + vec2(0.0, 1.0));
    float d = vc_channel_hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

vec4 vc_channel_sample(VcChannel channel, vec2 uv) {
    vec2 wrapped_uv = fract(uv);
    float channel_id = float(channel.id);
    vec2 p = wrapped_uv * 256.0 + channel_id * vec2(37.0, 17.0);
    float n0 = vc_channel_noise(p);
    float n1 = vc_channel_noise(p * 1.91 + vec2(19.1, 3.7));
    float n2 = vc_channel_noise(p * 3.07 + vec2(5.2, 41.3));
    if (channel.id == 1) {
        return vec4(n0, n0, n0, 1.0);
    }
    if (channel.id == 2) {
        return vec4(n0, n1, n2, 1.0);
    }
    if (channel.id == 3) {
        return vec4(vec3(step(0.5, n0)), 1.0);
    }
    return vec4(n0, n1, n2, 1.0);
}

vec4 texture(VcChannel channel, vec2 uv) {
    return vc_channel_sample(channel, uv);
}

vec4 textureLod(VcChannel channel, vec2 uv, float lod) {
    return vc_channel_sample(channel, uv + lod * 0.001);
}

vec4 texelFetch(VcChannel channel, ivec2 coord, int lod) {
    return vc_channel_sample(channel, (vec2(coord) + 0.5) / 256.0 + float(lod) * 0.001);
}
"#
}

pub fn fullscreen_vertex_wgsl() -> &'static str {
    r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0)
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 0.0)
    );
    var out: VertexOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}
"#
}

pub fn fullscreen_vertex_glsl() -> &'static str {
    r#"#version 450
layout(location = 0) out vec2 v_uv;

vec2 fullscreen_position(int vertex_index) {
    if (vertex_index == 0) {
        return vec2(-1.0, -3.0);
    }
    if (vertex_index == 1) {
        return vec2(3.0, 1.0);
    }
    return vec2(-1.0, 1.0);
}

vec2 fullscreen_uv(int vertex_index) {
    if (vertex_index == 0) {
        return vec2(0.0, 2.0);
    }
    if (vertex_index == 1) {
        return vec2(2.0, 0.0);
    }
    return vec2(0.0, 0.0);
}

void main() {
    int vertex_index = int(gl_VertexIndex);
    gl_Position = vec4(fullscreen_position(vertex_index), 0.0, 1.0);
    v_uv = fullscreen_uv(vertex_index);
}
"#
}
