vec4 video_creater_fragment(vec2 uv, float time, float progress) {
    return vc_original_mandelbulb_interior(uv, time, progress);
}
