//! Deterministic shader-style noise helpers for CPU-rendered graphics templates.
//!
//! These functions mirror the kind of reusable primitives we expect shader
//! authors to compose: smooth value noise, fBm, turbulence, domain warping, and
//! stable per-pixel random values. They stay allocation-free and deterministic
//! so renderer tests can compare visual metrics across runs.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainWarp2 {
    pub x: f64,
    pub y: f64,
    pub strength: f64,
}

pub fn fract(value: f64) -> f64 {
    value - value.floor()
}

pub fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let t = ((value - edge0) / (edge1 - edge0).max(f64::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn rotate_2d(x: f64, y: f64, angle: f64) -> (f64, f64) {
    let (sin, cos) = angle.sin_cos();
    (x * cos - y * sin, y * cos + x * sin)
}

pub fn value_noise_2d(x: f64, y: f64, seed: u32) -> f64 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let tx = smoothstep(0.0, 1.0, x - x0 as f64);
    let ty = smoothstep(0.0, 1.0, y - y0 as f64);

    let n00 = lattice_noise_2d(x0, y0, seed);
    let n10 = lattice_noise_2d(x0 + 1, y0, seed);
    let n01 = lattice_noise_2d(x0, y0 + 1, seed);
    let n11 = lattice_noise_2d(x0 + 1, y0 + 1, seed);
    let nx0 = n00 + (n10 - n00) * tx;
    let nx1 = n01 + (n11 - n01) * tx;
    nx0 + (nx1 - nx0) * ty
}

pub fn value_noise_3d(x: f64, y: f64, z: f64, seed: u32) -> f64 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smoothstep(0.0, 1.0, x - x0 as f64);
    let ty = smoothstep(0.0, 1.0, y - y0 as f64);
    let tz = smoothstep(0.0, 1.0, z - z0 as f64);

    let n000 = lattice_noise_3d(x0, y0, z0, seed);
    let n100 = lattice_noise_3d(x0 + 1, y0, z0, seed);
    let n010 = lattice_noise_3d(x0, y0 + 1, z0, seed);
    let n110 = lattice_noise_3d(x0 + 1, y0 + 1, z0, seed);
    let n001 = lattice_noise_3d(x0, y0, z0 + 1, seed);
    let n101 = lattice_noise_3d(x0 + 1, y0, z0 + 1, seed);
    let n011 = lattice_noise_3d(x0, y0 + 1, z0 + 1, seed);
    let n111 = lattice_noise_3d(x0 + 1, y0 + 1, z0 + 1, seed);

    let nx00 = n000 + (n100 - n000) * tx;
    let nx10 = n010 + (n110 - n010) * tx;
    let nx01 = n001 + (n101 - n001) * tx;
    let nx11 = n011 + (n111 - n011) * tx;
    let nxy0 = nx00 + (nx10 - nx00) * ty;
    let nxy1 = nx01 + (nx11 - nx01) * ty;
    nxy0 + (nxy1 - nxy0) * tz
}

pub fn fbm_2d(x: f64, y: f64, seed: u32, octaves: usize) -> f64 {
    let mut sum = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut norm = 0.0;

    for octave in 0..octaves {
        sum += value_noise_2d(x * frequency, y * frequency, seed + octave as u32 * 17) * amplitude;
        norm += amplitude;
        frequency *= 2.03;
        amplitude *= 0.5;
    }

    if norm <= f64::EPSILON {
        0.0
    } else {
        sum / norm
    }
}

pub fn fbm_3d(x: f64, y: f64, z: f64, seed: u32, octaves: usize) -> f64 {
    let mut sum = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut norm = 0.0;

    for octave in 0..octaves {
        sum += value_noise_3d(
            x * frequency,
            y * frequency,
            z * frequency,
            seed + octave as u32 * 19,
        ) * amplitude;
        norm += amplitude;
        frequency *= 2.01;
        amplitude *= 0.5;
    }

    if norm <= f64::EPSILON {
        0.0
    } else {
        sum / norm
    }
}

pub fn turbulence_3d(x: f64, y: f64, z: f64, seed: u32, octaves: usize) -> f64 {
    let mut sum = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut norm = 0.0;

    for octave in 0..octaves {
        let n = value_noise_3d(
            x * frequency,
            y * frequency,
            z * frequency,
            seed + octave as u32 * 23,
        );
        sum += (n * 2.0 - 1.0).abs() * amplitude;
        norm += amplitude;
        frequency *= 2.0;
        amplitude *= 0.54;
    }

    if norm <= f64::EPSILON {
        0.0
    } else {
        sum / norm
    }
}

pub fn domain_warp_2d(x: f64, y: f64, time: f64, seed: u32, strength: f64) -> DomainWarp2 {
    let qx = fbm_3d(x + 0.0, y + 0.0, time, seed, 4) * 2.0 - 1.0;
    let qy = fbm_3d(x + 5.2, y + 1.3, time + 2.7, seed + 31, 4) * 2.0 - 1.0;
    let rx = fbm_3d(
        x + qx * strength + 1.7,
        y + qy * strength + 9.2,
        time + 4.1,
        seed + 67,
        4,
    ) * 2.0
        - 1.0;
    let ry = fbm_3d(
        x + qx * strength + 8.3,
        y + qy * strength + 2.8,
        time + 6.4,
        seed + 101,
        4,
    ) * 2.0
        - 1.0;

    DomainWarp2 {
        x: x + rx * strength,
        y: y + ry * strength,
        strength: ((rx * rx + ry * ry).sqrt() * strength).clamp(0.0, strength * 1.5),
    }
}

pub fn stable_pixel_noise(x: u32, y: u32, frame: u32) -> f64 {
    let mut value = x.wrapping_mul(0x9e37_79b1)
        ^ y.wrapping_mul(0x85eb_ca6b).rotate_left(13)
        ^ frame.wrapping_mul(0xc2b2_ae35).rotate_left(7);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    (value >> 8) as f64 / 16_777_215.0
}

fn lattice_noise_2d(x: i32, y: i32, seed: u32) -> f64 {
    let mut value = (x as u32)
        .wrapping_mul(747_796_405)
        .wrapping_add((y as u32).wrapping_mul(2_891_336_453))
        .wrapping_add(seed.wrapping_mul(277_803_737));
    value ^= value >> 15;
    value = value.wrapping_mul(2_246_822_519);
    value ^= value >> 13;
    (value & 0xffff) as f64 / 65_535.0
}

fn lattice_noise_3d(x: i32, y: i32, z: i32, seed: u32) -> f64 {
    let mut value = (x as u32)
        .wrapping_mul(747_796_405)
        .wrapping_add((y as u32).wrapping_mul(2_891_336_453))
        .wrapping_add((z as u32).wrapping_mul(1_593_335_467))
        .wrapping_add(seed.wrapping_mul(277_803_737));
    value ^= value >> 15;
    value = value.wrapping_mul(2_246_822_519);
    value ^= value >> 13;
    value = value.wrapping_mul(3_266_489_917);
    value ^= value >> 16;
    (value & 0xffff) as f64 / 65_535.0
}
