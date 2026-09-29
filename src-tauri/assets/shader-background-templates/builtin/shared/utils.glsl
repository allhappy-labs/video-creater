const float VC_PI = 3.14159265359;
const float VC_TAU = 6.28318530718;

mat2 vc_rot(float a) {
    float c = cos(a);
    float s = sin(a);
    return mat2(c, s, -s, c);
}

float vc_hash11(float n) {
    return fract(sin(n) * 43758.5453123);
}

float vc_hash21(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453123);
}

vec2 vc_hash22(vec2 p) {
    vec2 q = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return fract(sin(q) * 43758.5453123);
}

float vc_noise2(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = vc_hash21(i);
    float b = vc_hash21(i + vec2(1.0, 0.0));
    float c = vc_hash21(i + vec2(0.0, 1.0));
    float d = vc_hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float vc_noise3(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float n = dot(i, vec3(1.0, 57.0, 113.0));
    float a = vc_hash11(n);
    float b = vc_hash11(n + 1.0);
    float c = vc_hash11(n + 57.0);
    float d = vc_hash11(n + 58.0);
    float e = vc_hash11(n + 113.0);
    float g = vc_hash11(n + 114.0);
    float h = vc_hash11(n + 170.0);
    float j = vc_hash11(n + 171.0);
    return mix(mix(mix(a, b, f.x), mix(c, d, f.x), f.y), mix(mix(e, g, f.x), mix(h, j, f.x), f.y), f.z);
}

float vc_fbm2(vec2 p) {
    float f = 0.0;
    float a = 0.5;
    mat2 m = mat2(0.80, 0.60, -0.60, 0.80);
    for (int i = 0; i < 5; i++) {
        f += a * vc_noise2(p);
        p = m * p * 2.02;
        a *= 0.5;
    }
    return f;
}

float vc_fbm3(vec3 p) {
    float f = 0.0;
    float a = 0.5;
    for (int i = 0; i < 5; i++) {
        f += a * vc_noise3(p);
        p = p * 2.03 + vec3(7.1, 3.7, 5.3);
        a *= 0.5;
    }
    return f;
}

vec3 vc_palette(float t) {
    return 0.5 + 0.5 * cos(VC_TAU * (t + vec3(0.00, 0.33, 0.67)));
}

float vc_sdbox(vec3 p, vec3 b) {
    vec3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

float vc_sdroundbox(vec3 p, vec3 b, float r) {
    return vc_sdbox(p, b) - r;
}

float vc_line(float value, float width) {
    return 1.0 - smoothstep(0.0, width, abs(fract(value) - 0.5));
}

float vc_hex(vec2 p) {
    p = abs(p);
    return max(dot(p, vec2(0.8660254, 0.5)), p.y);
}

float vc_star5(vec2 p, float r, float rf) {
    float a = atan(p.y, p.x);
    float l = length(p);
    float k = cos(floor(0.5 + a / VC_TAU * 5.0) * VC_TAU / 5.0 - a);
    return l * k - mix(r, r * rf, 0.5 + 0.5 * cos(5.0 * a));
}

vec3 vc_aces(vec3 v) {
    v = max(v, 0.0);
    v *= 0.6;
    float a = 2.51;
    float b = 0.03;
    float c = 2.43;
    float d = 0.59;
    float e = 0.14;
    return clamp((v * (a * v + b)) / (v * (c * v + d) + e), 0.0, 1.0);
}

vec2 vc_screen(vec2 uv) {
    vec2 p = uv * 2.0 - 1.0;
    p.x *= u_resolution.x / max(u_resolution.y, 1.0);
    return p;
}

float vc_ring(float d, float radius, float width) {
    return 1.0 - smoothstep(width, width * 2.2, abs(d - radius));
}

float vc_segment_line(vec2 p, vec2 a, vec2 b, float width) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 0.0001), 0.0, 1.0);
    return 1.0 - smoothstep(width, width * 2.5, length(pa - ba * h));
}

vec3 vc_heat_palette(float v) {
    vec3 a = vec3(0.035, 0.045, 0.080);
    vec3 b = vec3(0.070, 0.310, 0.540);
    vec3 c = vec3(1.000, 0.430, 0.170);
    vec3 d = vec3(1.000, 0.850, 0.450);
    return mix(mix(a, b, smoothstep(0.0, 0.45, v)), mix(c, d, smoothstep(0.65, 1.0, v)), smoothstep(0.38, 0.9, v));
}

vec4 vc_original_octagram_tunnel(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 col = vec3(0.015, 0.030, 0.070);
    float t = time * 0.42;
    for (int i = 0; i < 7; i++) {
        float fi = float(i);
        float z = fi * 0.42 + fract(t + fi * 0.137);
        vec2 q = p / (0.25 + z * 1.6);
        q *= vc_rot(t * 0.55 + fi * 0.39);
        q = abs(q);
        float boxA = abs(max(q.x, q.y) - (0.42 + 0.06 * sin(time + fi)));
        q *= vc_rot(0.785398);
        float boxB = abs(max(abs(q.x), abs(q.y)) - 0.42);
        float line = exp(-24.0 * min(boxA, boxB));
        float fade = (1.0 - z) * (1.0 - z);
        col += fade * line * vec3(0.35, 0.76, 1.0);
        col += fade * exp(-7.0 * length(p)) * vec3(0.02, 0.22, 0.45);
    }
    float grain = vc_fbm2(p * 18.0 + time * 0.15);
    col += 0.08 * grain * vec3(0.25, 0.45, 0.9);
    return vec4(vc_aces(col * 1.4), 1.0);
}

vec4 vc_original_base_warp(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec2 flow = p;
    flow.x += 0.35 * sin(flow.y * 2.4 + time * 0.55);
    flow.y += 0.25 * cos(flow.x * 2.0 - time * 0.35);
    float a = vc_fbm2(flow * 1.7 + vec2(time * 0.08, -time * 0.04));
    float b = vc_fbm2(flow * 3.3 + a * 2.0 - time * 0.06);
    float v = smoothstep(0.15, 0.95, a * 0.65 + b * 0.55);
    vec3 col = mix(vec3(0.11, 0.04, 0.22), vec3(1.0, 0.18, 0.82), v);
    col = mix(col, vec3(1.0, 0.82, 0.96), smoothstep(0.58, 1.08, b + p.x * 0.1));
    return vec4(col, 1.0);
}

vec4 vc_original_phantom_star(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 col = vec3(0.015, 0.020, 0.035);
    for (int i = 0; i < 6; i++) {
        float fi = float(i);
        vec2 q = p * (1.0 + fi * 0.22);
        q *= vc_rot(time * 0.16 + fi * 0.63);
        float d = abs(vc_star5(q, 0.35 + 0.07 * fi, 0.42));
        float ray = exp(-22.0 * d) * (1.0 - fi * 0.11);
        col += ray * vec3(0.24, 0.88, 1.0);
        col += exp(-5.0 * abs(q.x)) * exp(-3.0 * length(q)) * 0.03;
    }
    col *= 1.0 - 0.25 * length(p);
    return vec4(vc_aces(col * 2.0), 1.0);
}

vec4 vc_original_volume_clouds(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 skyTop = vec3(0.56, 0.72, 0.86);
    vec3 skyLow = vec3(1.0, 0.82, 0.52);
    float horizon = smoothstep(-0.85, 0.65, p.y);
    vec3 col = mix(skyLow, skyTop, horizon);
    vec2 drift = vec2(time * 0.035, -time * 0.012);
    float cover = 0.0;
    float shade = 0.0;
    for (int i = 0; i < 5; i++) {
        float fi = float(i);
        vec2 q = p * (1.25 + fi * 0.75) + drift * (1.0 + fi);
        q.x += sin(p.y * 2.0 + time * 0.08) * 0.18;
        float n = vc_fbm2(q + fi * 13.7);
        cover += n * (0.42 / (1.0 + fi * 0.48));
        shade += vc_noise2(q * 2.1 + 8.0) * (0.28 / (1.0 + fi));
    }
    float billow = vc_fbm2(p * 5.0 + vec2(-time * 0.025, time * 0.02));
    float cloudMask = smoothstep(0.20, 0.78, cover + billow * 0.28 + p.y * 0.42 + 0.16);
    float lowerFade = smoothstep(-0.92, -0.34, p.y);
    float topFade = 1.0 - smoothstep(1.05, 1.55, p.y);
    float density = cloudMask * lowerFade * topFade;
    vec3 cloudWarm = vec3(1.0, 0.91, 0.75);
    vec3 cloudCool = vec3(0.42, 0.52, 0.58);
    float light = clamp(0.48 + p.x * -0.30 + p.y * 0.20 + cover * 0.35, 0.0, 1.0);
    vec3 cloud = mix(cloudCool, cloudWarm, light);
    cloud *= 0.62 + 0.58 * smoothstep(0.16, 0.88, cover) - 0.34 * shade;
    cloud += vec3(1.0, 0.72, 0.40) * exp(-4.2 * length(p - vec2(-0.88, 0.02))) * 0.24;
    col = mix(col, cloud, density * 0.96);
    col -= density * (1.0 - light) * vec3(0.10, 0.12, 0.14);
    return vec4(clamp(col, 0.0, 1.0), 1.0);
}

vec4 vc_original_cube_lines(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 col = vec3(0.035, 0.055, 0.075);
    vec2 q = p * vc_rot(0.12 * sin(time * 0.3));
    float frame = max(abs(q.x * 0.78 + q.y * 0.18), abs(q.y * 0.78 - q.x * 0.18));
    col += vc_ring(frame, 0.56, 0.018) * vec3(0.2, 0.85, 1.0);
    for (int i = 0; i < 9; i++) {
        float fi = float(i);
        float y = -0.45 + fi * 0.11 + 0.03 * sin(time + fi);
        float l = vc_segment_line(q, vec2(-0.42, y), vec2(0.42, -y * 0.6), 0.006);
        col += l * vec3(0.28, 0.78, 1.0) * (0.45 + 0.55 * sin(fi + time));
    }
    col += vec3(1.0, 0.45, 0.15) * exp(-6.0 * abs(frame - 0.42)) * 0.16;
    return vec4(vc_aces(col * 2.3), 1.0);
}

vec4 vc_original_crumpled_wave(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    float h = 0.0;
    for (int i = 0; i < 6; i++) {
        float fi = float(i);
        h += sin(p.x * (2.0 + fi) + cos(p.y * (1.7 + fi * 0.4) + time * 0.25) * 2.0 + time * (0.25 + fi * 0.04)) / (1.6 + fi);
    }
    float bands = 1.0 - smoothstep(0.02, 0.12, abs(fract(h * 0.55 + p.y * 0.35) - 0.5));
    vec3 col = mix(vec3(0.03, 0.09, 0.18), vec3(0.14, 0.78, 1.0), smoothstep(-1.0, 1.4, h));
    col += bands * vec3(0.18, 0.9, 1.0);
    return vec4(vc_aces(col * 1.5), 1.0);
}

vec4 vc_original_marbling(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec2 q = p;
    q.x += 0.42 * sin(q.y * 3.0 + time * 0.25);
    q.y += 0.32 * cos(q.x * 2.2 - time * 0.19);
    float f = vc_fbm2(q * 2.5 + time * 0.04);
    float vein = abs(sin((q.x + q.y * 0.6 + f * 1.8) * 8.0));
    float white = smoothstep(0.78, 0.98, vein);
    float fine = smoothstep(0.70, 0.92, abs(sin((q.x * 1.7 - q.y + f) * 22.0)));
    vec3 col = vec3(0.16 + 0.28 * f);
    col += white * vec3(0.72);
    col += fine * vec3(0.18);
    return vec4(clamp(col, 0.0, 1.0), 1.0);
}

vec4 vc_original_geodesic(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    p *= vc_rot(time * 0.12);
    float r = length(p);
    vec3 col = vec3(0.0, 0.16, 0.25);
    if (r < 0.72) {
        vec2 sphere = p / 0.72;
        float z = sqrt(max(0.0, 1.0 - dot(sphere, sphere)));
        vec3 n = normalize(vec3(sphere, z));
        vec2 g = vec2(atan(n.z, n.x) / VC_TAU, asin(n.y) / VC_PI) * vec2(18.0, 10.0);
        g += vec2(time * 0.06, sin(time * 0.2) * 0.18);
        float cell = min(abs(fract(g.x) - 0.5), abs(fract(g.y + 0.5 * fract(g.x)) - 0.5));
        float seam = 1.0 - smoothstep(0.015, 0.045, cell);
        vec3 base = mix(vec3(0.70, 0.95, 0.92), vec3(0.95, 0.86, 1.0), n.x * 0.5 + 0.5);
        col = base * (0.55 + 0.45 * z) + seam * vec3(0.1, 0.9, 1.0);
        col += seam * vec3(1.0, 0.15, 0.9) * smoothstep(0.1, 0.9, sin(g.x * 0.9 + time) * 0.5 + 0.5);
    }
    return vec4(clamp(col, 0.0, 1.0), 1.0);
}

vec4 vc_original_kaleidoscope(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    float a = atan(p.y, p.x);
    float r = length(p);
    float sector = abs(fract(a / VC_TAU * 8.0 + 0.5) - 0.5);
    float tunnel = fract(1.4 / max(r, 0.05) + time * 0.35);
    float motif = 1.0 - smoothstep(0.015, 0.09, min(sector, abs(tunnel - 0.5)));
    float dust = smoothstep(0.62, 0.95, vc_fbm2(p * 18.0 + time * 0.15));
    vec3 col = vec3(0.02, 0.025, 0.06);
    col += motif * vec3(0.58, 0.70, 1.0) * (1.2 - r);
    col += dust * vec3(0.5, 0.8, 1.0) * 0.42;
    return vec4(vc_aces(col * 2.0), 1.0);
}

vec4 vc_original_digital_brain(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv) * 2.0;
    vec2 g = floor(p * 4.0);
    vec2 f = fract(p * 4.0) - 0.5;
    float cell = vc_hash21(g);
    float node = 1.0 - smoothstep(0.02, 0.16, length(f - (vc_hash22(g) - 0.5) * 0.45));
    float trace = 1.0 - smoothstep(0.015, 0.055, min(abs(f.x), abs(f.y)));
    float pulse = smoothstep(0.35, 1.0, sin(time * 2.0 + cell * 8.0) * 0.5 + 0.5);
    vec3 col = vec3(0.015, 0.030, 0.070);
    col += trace * vec3(0.0, 0.35, 0.9) * (0.2 + 0.7 * cell);
    col += node * pulse * vec3(1.0, 0.48, 0.12);
    col *= 1.0 - 0.18 * length(p * 0.5);
    return vec4(vc_aces(col * 2.2), 1.0);
}

vec4 vc_original_starry_planes(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 col = vec3(0.01, 0.01, 0.025);
    for (int i = 0; i < 8; i++) {
        float fi = float(i);
        float z = fract(time * 0.15 + fi / 8.0);
        vec2 q = p / (0.22 + z * 1.8);
        q *= vc_rot(fi * 1.7 + time * 0.12);
        float star = exp(-28.0 * abs(vc_star5(q, 0.34, 0.45)));
        col += star * (1.0 - z) * vec3(0.85, 0.32, 1.0);
    }
    col += smoothstep(0.75, 1.0, vc_fbm2(p * 12.0 + time)) * vec3(0.25, 0.55, 1.0);
    return vec4(vc_aces(col * 1.8), 1.0);
}

vec4 vc_original_mandelbulb_interior(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    float r = length(p);
    float a = atan(p.y, p.x);
    float spiral = sin(7.0 * log(r + 0.08) - a * 4.0 + time * 0.45);
    float folds = vc_fbm2(vec2(a * 1.8, log(r + 0.15) * 2.5) + time * 0.04);
    float core = exp(-4.0 * r);
    vec3 amber = vec3(1.0, 0.48, 0.08);
    vec3 blue = vec3(0.02, 0.13, 0.45);
    vec3 col = mix(blue, amber, smoothstep(-0.35, 0.75, spiral + folds * 0.8));
    col += core * vec3(0.05, 0.5, 0.9);
    col *= 0.8 + 0.6 * folds;
    return vec4(vc_aces(col * 1.7), 1.0);
}

vec4 vc_original_tiny_clouds(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 sky = mix(vec3(0.72, 0.86, 1.0), vec3(1.0, 0.86, 0.58), smoothstep(-1.0, 0.8, -p.x - p.y));
    float f = vc_fbm2(p * 2.4 + vec2(time * 0.04, 0.0));
    float cloud = smoothstep(0.42, 0.78, f + 0.25 - length(p * vec2(0.7, 1.1)) * 0.25);
    vec3 col = mix(sky, vec3(0.92, 0.96, 1.0), cloud * 0.75);
    col -= cloud * vc_fbm2(p * 8.0) * 0.12;
    return vec4(clamp(col, 0.0, 1.0), 1.0);
}

vec4 vc_original_neon_hex(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    float depth = clamp((p.y + 1.15) / 2.2, 0.0, 1.0);
    vec2 plane = vec2(p.x / (0.35 + depth * 1.55), p.y * 2.0 + time * 0.22);
    vec2 grid = vec2(plane.x * 3.2, plane.y * 2.1);
    vec2 a = vec2(grid.x + 0.5 * floor(grid.y), grid.y);
    vec2 cell = fract(a) - 0.5;
    vec2 id = floor(a);
    float edge = abs(vc_hex(cell) - 0.38);
    float seam = 1.0 - smoothstep(0.014, 0.045, edge);
    float lampGate = smoothstep(0.58, 0.82, vc_hash21(id));
    float lamp = seam * lampGate;
    float tileShade = 0.42 + 0.12 * vc_fbm2(id * 0.17) + 0.20 * depth;
    vec3 stone = mix(vec3(0.18, 0.22, 0.25), vec3(0.42, 0.48, 0.52), tileShade);
    stone *= 0.90 + 0.10 * smoothstep(0.0, 0.38, vc_hex(cell));
    vec3 glow = vec3(1.0, 0.20, 1.0) * lamp * 2.2 + vec3(0.45, 0.22, 1.0) * lamp * 0.9;
    glow += exp(-36.0 * edge) * lampGate * vec3(0.8, 0.18, 1.0) * 0.85;
    float haze = smoothstep(0.0, 1.0, depth) * 0.18;
    vec3 col = stone + glow + haze * vec3(0.18, 0.12, 0.25);
    col *= 1.0 - 0.28 * smoothstep(0.75, 1.45, length(p));
    return vec4(vc_aces(col * 1.25), 1.0);
}

vec4 vc_original_another_cube(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec2 q = p * vc_rot(time * 0.25);
    float box = max(abs(q.x), abs(q.y));
    float cube = 1.0 - smoothstep(0.42, 0.48, box);
    float rim = vc_ring(box, 0.46, 0.015);
    float orb = vc_ring(length(q - vec2(0.02, 0.02)), 0.24, 0.015);
    vec3 col = vec3(0.20, 0.26, 0.42);
    col += cube * vec3(0.08, 0.12, 0.20);
    col += rim * vec3(0.2, 0.65, 1.0);
    col += orb * vec3(0.85, 0.95, 1.0) * 1.8;
    col += exp(-5.0 * abs(length(q) - 0.25)) * vec3(0.15, 0.25, 0.45);
    return vec4(vc_aces(col * 1.5), 1.0);
}

vec4 vc_original_object_mesh(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec2 q = p * vc_rot(0.35 + 0.15 * sin(time));
    float shape = smoothstep(0.78, 0.34, max(abs(q.x * 0.8 + q.y * 0.25), abs(q.y * 0.9 - q.x * 0.2)));
    float cuts = 0.0;
    for (int i = 0; i < 7; i++) {
        float fi = float(i);
        vec2 n = normalize(vec2(cos(fi * 2.17), sin(fi * 1.73)));
        cuts += 1.0 - smoothstep(0.015, 0.05, abs(dot(q, n) - 0.18 * sin(fi + time * 0.2)));
    }
    float metal = 0.45 + 0.35 * vc_fbm2(q * 7.0);
    vec3 col = mix(vec3(0.42, 0.45, 0.48), vec3(0.9, 0.78, 0.48), metal);
    col = mix(vec3(0.62), col + cuts * vec3(0.2, 0.35, 0.45), shape);
    return vec4(clamp(col, 0.0, 1.0), 1.0);
}

vec4 vc_original_looping_clouds(vec2 uv, float time, float progress) {
    vec2 p = vc_screen(uv);
    vec3 col = vec3(0.01, 0.015, 0.025);
    for (int i = 0; i < 9; i++) {
        float fi = float(i);
        float z = fract(progress + fi / 9.0);
        vec2 q = p / (0.25 + z * 1.7);
        q += vec2(sin(time * 0.2 + fi), cos(time * 0.17 + fi)) * 0.12;
        float cloud = smoothstep(0.52, 0.92, vc_fbm2(q * 2.0 + fi));
        float line = vc_ring(abs(q.x) + 0.3 * sin(q.y * 3.0 + time), 0.42, 0.025);
        vec3 pal = vc_palette(fi * 0.13 + z * 0.4);
        col += (cloud * 0.12 + line * 0.45) * pal * (1.0 - z);
    }
    return vec4(vc_aces(col * 2.0), 1.0);
}
