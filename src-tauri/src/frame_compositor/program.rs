use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
thread_local! {
    static BILINEAR_SAMPLE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyframeEasing {
    Linear,
    Hold,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericKeyframe {
    pub at_seconds: f64,
    pub value: f64,
    pub easing: KeyframeEasing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericCurve {
    pub base: f64,
    pub keyframes: Vec<NumericKeyframe>,
}

impl NumericCurve {
    pub fn constant(value: f64) -> Self {
        Self {
            base: value,
            keyframes: Vec::new(),
        }
    }

    pub fn sample(&self, seconds: f64) -> f64 {
        let Some(first) = self.keyframes.first() else {
            return self.base;
        };
        if seconds <= first.at_seconds {
            return first.value;
        }
        let Some(last) = self.keyframes.last() else {
            return self.base;
        };
        if seconds >= last.at_seconds {
            return last.value;
        }
        for pair in self.keyframes.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            if seconds > right.at_seconds {
                continue;
            }
            let span = right.at_seconds - left.at_seconds;
            let progress = if span <= f64::EPSILON {
                0.0
            } else {
                ((seconds - left.at_seconds) / span).clamp(0.0, 1.0)
            };
            let eased = match left.easing {
                KeyframeEasing::Linear => progress,
                KeyframeEasing::Hold => 0.0,
                KeyframeEasing::EaseIn => progress * progress,
                KeyframeEasing::EaseOut => 1.0 - (1.0 - progress) * (1.0 - progress),
                KeyframeEasing::EaseInOut => progress * progress * (3.0 - 2.0 * progress),
            };
            return left.value + (right.value - left.value) * eased;
        }
        last.value
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasTransform {
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameProgram {
    pub duration_seconds: f64,
    pub canvas_transform: CanvasTransform,
    pub position_x: NumericCurve,
    pub position_y: NumericCurve,
    pub scale_x: NumericCurve,
    pub scale_y: NumericCurve,
    pub rotation_degrees: NumericCurve,
    pub opacity: NumericCurve,
    pub crop_top: NumericCurve,
    pub crop_right: NumericCurve,
    pub crop_bottom: NumericCurve,
    pub crop_left: NumericCurve,
    pub fade_in_seconds: f64,
    pub fade_out_seconds: f64,
}

impl FrameProgram {
    pub fn identity(duration_seconds: f64) -> Self {
        Self {
            duration_seconds,
            canvas_transform: CanvasTransform::default(),
            position_x: NumericCurve::constant(0.0),
            position_y: NumericCurve::constant(0.0),
            scale_x: NumericCurve::constant(1.0),
            scale_y: NumericCurve::constant(1.0),
            rotation_degrees: NumericCurve::constant(0.0),
            opacity: NumericCurve::constant(1.0),
            crop_top: NumericCurve::constant(0.0),
            crop_right: NumericCurve::constant(0.0),
            crop_bottom: NumericCurve::constant(0.0),
            crop_left: NumericCurve::constant(0.0),
            fade_in_seconds: 0.0,
            fade_out_seconds: 0.0,
        }
    }

    pub fn sample(&self, local_seconds: f64) -> Result<SampledFrameProgram, FrameProgramError> {
        let local_seconds = local_seconds.clamp(0.0, self.duration_seconds.max(0.0));
        let fade_in = if self.fade_in_seconds > 0.0 {
            (local_seconds / self.fade_in_seconds).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let fade_out = if self.fade_out_seconds > 0.0 {
            ((self.duration_seconds - local_seconds) / self.fade_out_seconds).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let crop_top = self.crop_top.sample(local_seconds);
        let crop_right = self.crop_right.sample(local_seconds);
        let crop_bottom = self.crop_bottom.sample(local_seconds);
        let crop_left = self.crop_left.sample(local_seconds);
        if crop_left < 0.0
            || crop_right < 0.0
            || crop_top < 0.0
            || crop_bottom < 0.0
            || crop_left + crop_right >= 1.0
            || crop_top + crop_bottom >= 1.0
        {
            return Err(FrameProgramError::InvalidCrop);
        }
        let scale_x = self.scale_x.sample(local_seconds);
        let scale_y = self.scale_y.sample(local_seconds);
        if !scale_x.is_finite() || !scale_y.is_finite() || scale_x <= 0.0 || scale_y <= 0.0 {
            return Err(FrameProgramError::InvalidScale);
        }
        Ok(SampledFrameProgram {
            canvas_transform: self.canvas_transform.clone(),
            position_x: self.position_x.sample(local_seconds),
            position_y: self.position_y.sample(local_seconds),
            scale_x,
            scale_y,
            rotation_degrees: self.rotation_degrees.sample(local_seconds),
            opacity: (self.opacity.sample(local_seconds) * fade_in * fade_out).clamp(0.0, 1.0),
            crop_top,
            crop_right,
            crop_bottom,
            crop_left,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampledFrameProgram {
    pub canvas_transform: CanvasTransform,
    pub position_x: f64,
    pub position_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_degrees: f64,
    pub opacity: f64,
    pub crop_top: f64,
    pub crop_right: f64,
    pub crop_bottom: f64,
    pub crop_left: f64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameProgramError {
    #[error("frame dimensions or RGBA buffer length are invalid")]
    InvalidDimensions,
    #[error("frame program scale must remain positive and finite")]
    InvalidScale,
    #[error("frame program crop must leave visible content")]
    InvalidCrop,
}

pub fn transform_rgba8_srgb(
    source: &[u8],
    width: u32,
    height: u32,
    program: &SampledFrameProgram,
) -> Result<Vec<u8>, FrameProgramError> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(FrameProgramError::InvalidDimensions)?;
    if width == 0 || height == 0 || source.len() != expected {
        return Err(FrameProgramError::InvalidDimensions);
    }
    let transform = &program.canvas_transform;
    if !transform.center_x.is_finite()
        || !transform.center_y.is_finite()
        || !transform.width.is_finite()
        || !transform.height.is_finite()
        || transform.width <= 0.0
        || transform.height <= 0.0
        || !program.position_x.is_finite()
        || !program.position_y.is_finite()
        || !program.rotation_degrees.is_finite()
    {
        return Err(FrameProgramError::InvalidDimensions);
    }
    let mut output = vec![0_u8; expected];
    let center_x = transform.center_x * f64::from(width) + program.position_x;
    let center_y = transform.center_y * f64::from(height) + program.position_y;
    let output_width = f64::from(width) * transform.width * program.scale_x;
    let output_height = f64::from(height) * transform.height * program.scale_y;
    if output_width <= 0.0 || output_height <= 0.0 {
        return Err(FrameProgramError::InvalidScale);
    }
    if transform == &CanvasTransform::default()
        && program.position_x == 0.0
        && program.position_y == 0.0
        && program.scale_x == 1.0
        && program.scale_y == 1.0
        && program.rotation_degrees == 0.0
        && program.crop_top == 0.0
        && program.crop_right == 0.0
        && program.crop_bottom == 0.0
        && program.crop_left == 0.0
    {
        // Floating coordinate cancellation can still put an identity sample
        // into an adjacent pixel on odd rasters. Skip interpolation only when
        // the original arithmetic recovers the exact source pixel center.
        let columns = (0..width)
            .map(|x| {
                let normalized = (f64::from(x) + 0.5 - center_x) / output_width + 0.5;
                let exact = normalized * f64::from(width) - 0.5 == f64::from(x);
                (normalized, exact)
            })
            .collect::<Vec<_>>();
        for y in 0..height {
            let normalized_y = (f64::from(y) + 0.5 - center_y) / output_height + 0.5;
            let exact_y = normalized_y * f64::from(height) - 0.5 == f64::from(y);
            for (x, &(normalized_x, exact_x)) in columns.iter().enumerate() {
                let sampled = if exact_x && exact_y {
                    let original = pixel(source, width, x as u32, y);
                    if original[3] == 0 {
                        [0; 4]
                    } else {
                        original
                    }
                } else {
                    bilinear_sample(source, width, height, normalized_x, normalized_y)
                };
                let offset = (y as usize * width as usize + x) * 4;
                output[offset..offset + 4]
                    .copy_from_slice(&apply_opacity(sampled, program.opacity));
            }
        }
        return Ok(output);
    }
    let radians = program.rotation_degrees.to_radians();
    let cosine = radians.cos();
    let sine = radians.sin();
    for y in 0..height {
        for x in 0..width {
            let dx = f64::from(x) + 0.5 - center_x;
            let dy = f64::from(y) + 0.5 - center_y;
            let local_x = cosine * dx + sine * dy;
            let local_y = -sine * dx + cosine * dy;
            let mut normalized_x = local_x / output_width + 0.5;
            let mut normalized_y = local_y / output_height + 0.5;
            if normalized_x < program.crop_left
                || normalized_x >= 1.0 - program.crop_right
                || normalized_y < program.crop_top
                || normalized_y >= 1.0 - program.crop_bottom
            {
                continue;
            }
            if transform.flip_horizontal {
                normalized_x = 1.0 - normalized_x;
            }
            if transform.flip_vertical {
                normalized_y = 1.0 - normalized_y;
            }
            let sampled = bilinear_sample(source, width, height, normalized_x, normalized_y);
            let offset = ((y * width + x) * 4) as usize;
            output[offset..offset + 4].copy_from_slice(&apply_opacity(sampled, program.opacity));
        }
    }
    Ok(output)
}

fn bilinear_sample(source: &[u8], width: u32, height: u32, x: f64, y: f64) -> [u8; 4] {
    #[cfg(test)]
    BILINEAR_SAMPLE_COUNT.with(|count| count.set(count.get() + 1));
    let source_x = x * f64::from(width) - 0.5;
    let source_y = y * f64::from(height) - 0.5;
    let x0 = source_x.floor().clamp(0.0, f64::from(width - 1)) as u32;
    let y0 = source_y.floor().clamp(0.0, f64::from(height - 1)) as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let tx = (source_x - source_x.floor()).clamp(0.0, 1.0) as f32;
    let ty = (source_y - source_y.floor()).clamp(0.0, 1.0) as f32;
    let pixels = [
        linear_premultiplied(pixel(source, width, x0, y0)),
        linear_premultiplied(pixel(source, width, x1, y0)),
        linear_premultiplied(pixel(source, width, x0, y1)),
        linear_premultiplied(pixel(source, width, x1, y1)),
    ];
    let top = lerp_pixel(pixels[0], pixels[1], tx);
    let bottom = lerp_pixel(pixels[2], pixels[3], tx);
    encode_linear_premultiplied(lerp_pixel(top, bottom, ty))
}

fn pixel(source: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    source[offset..offset + 4].try_into().expect("RGBA pixel")
}

fn apply_opacity(mut pixel: [u8; 4], opacity: f64) -> [u8; 4] {
    pixel[3] = (f64::from(pixel[3]) * opacity.clamp(0.0, 1.0)).round() as u8;
    pixel
}

fn linear_premultiplied(pixel: [u8; 4]) -> [f32; 4] {
    let alpha = f32::from(pixel[3]) / 255.0;
    [
        srgb_to_linear(f32::from(pixel[0]) / 255.0) * alpha,
        srgb_to_linear(f32::from(pixel[1]) / 255.0) * alpha,
        srgb_to_linear(f32::from(pixel[2]) / 255.0) * alpha,
        alpha,
    ]
}

fn encode_linear_premultiplied(pixel: [f32; 4]) -> [u8; 4] {
    if pixel[3] <= f32::EPSILON {
        return [0, 0, 0, 0];
    }
    let channel =
        |value: f32| (linear_to_srgb((value / pixel[3]).clamp(0.0, 1.0)) * 255.0).round() as u8;
    [
        channel(pixel[0]),
        channel(pixel[1]),
        channel(pixel[2]),
        (pixel[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn lerp_pixel(left: [f32; 4], right: [f32; 4], amount: f32) -> [f32; 4] {
    [
        left[0] + (right[0] - left[0]) * amount,
        left[1] + (right[1] - left[1]) * amount,
        left[2] + (right[2] - left[2]) * amount,
        left[3] + (right[3] - left[3]) * amount,
    ]
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_geometry_preserves_every_channel_alpha_pair_and_skips_exact_samples() {
        for (width, height) in [(256, 256), (257, 255)] {
            let source = (0..width * height)
                .flat_map(|index| {
                    let value = index as u8;
                    [value, 255 - value, value / 2, (index / 256) as u8]
                })
                .collect::<Vec<_>>();
            for opacity in [0.0, 0.003, 0.5, 0.73, 1.0] {
                let mut program = FrameProgram::identity(1.0);
                program.opacity = NumericCurve::constant(opacity);
                let sampled = program.sample(0.0).expect("sampled identity");
                let mut expected = Vec::with_capacity(source.len());
                for y in 0..height {
                    for x in 0..width {
                        // Use the original geometric coordinate arithmetic,
                        // including cancellation on odd, non-power-of-two rasters.
                        let normalized_x =
                            (f64::from(x) + 0.5 - f64::from(width) * 0.5) / f64::from(width) + 0.5;
                        let normalized_y = (f64::from(y) + 0.5 - f64::from(height) * 0.5)
                            / f64::from(height)
                            + 0.5;
                        expected.extend_from_slice(&apply_opacity(
                            bilinear_sample(&source, width, height, normalized_x, normalized_y),
                            sampled.opacity,
                        ));
                    }
                }
                BILINEAR_SAMPLE_COUNT.with(|count| count.set(0));
                let output = transform_rgba8_srgb(&source, width, height, &sampled)
                    .expect("transformed identity");
                let mismatch = output.iter().zip(&expected).position(|(a, b)| a != b);
                assert_eq!(
                    mismatch, None,
                    "{width}x{height}, opacity {opacity}: first mismatched byte"
                );
                assert!(
                    BILINEAR_SAMPLE_COUNT.with(std::cell::Cell::get) < (width * height) as usize,
                    "identity geometry must skip exact pixel-center samples"
                );
            }
        }
    }

    #[test]
    fn every_geometry_change_retains_resampling() {
        let source = (0..16_u8)
            .flat_map(|y| (0..16_u8).flat_map(move |x| [x * 16, y * 16, x ^ y, 255]))
            .collect::<Vec<_>>();
        let identity = FrameProgram::identity(1.0).sample(0.0).expect("identity");
        type GeometryChange = (&'static str, fn(&mut SampledFrameProgram));
        let changes: [GeometryChange; 15] = [
            ("canvas center x", |p| p.canvas_transform.center_x = 0.25),
            ("canvas center y", |p| p.canvas_transform.center_y = 0.25),
            ("canvas width", |p| p.canvas_transform.width = 0.75),
            ("canvas height", |p| p.canvas_transform.height = 0.75),
            ("horizontal flip", |p| {
                p.canvas_transform.flip_horizontal = true
            }),
            ("vertical flip", |p| p.canvas_transform.flip_vertical = true),
            ("position x", |p| p.position_x = 2.0),
            ("position y", |p| p.position_y = 2.0),
            ("scale x", |p| p.scale_x = 0.75),
            ("scale y", |p| p.scale_y = 0.75),
            ("rotation", |p| p.rotation_degrees = 90.0),
            ("crop top", |p| p.crop_top = 0.25),
            ("crop right", |p| p.crop_right = 0.25),
            ("crop bottom", |p| p.crop_bottom = 0.25),
            ("crop left", |p| p.crop_left = 0.25),
        ];
        for (name, change) in changes {
            let mut sampled = identity.clone();
            change(&mut sampled);
            BILINEAR_SAMPLE_COUNT.with(|count| count.set(0));
            let output =
                transform_rgba8_srgb(&source, 16, 16, &sampled).expect("transformed geometry");
            assert_ne!(output, source, "{name} must change pixels");
            assert!(
                BILINEAR_SAMPLE_COUNT.with(std::cell::Cell::get) > 0,
                "{name} must evaluate the bilinear sampler"
            );
        }
    }

    #[test]
    fn curves_apply_easing_and_hold_deterministically() {
        let curve = NumericCurve {
            base: 0.0,
            keyframes: vec![
                NumericKeyframe {
                    at_seconds: 0.0,
                    value: 0.0,
                    easing: KeyframeEasing::EaseInOut,
                },
                NumericKeyframe {
                    at_seconds: 1.0,
                    value: 10.0,
                    easing: KeyframeEasing::Hold,
                },
                NumericKeyframe {
                    at_seconds: 2.0,
                    value: 20.0,
                    easing: KeyframeEasing::Linear,
                },
            ],
        };
        assert_eq!(curve.sample(0.5), 5.0);
        assert_eq!(curve.sample(1.5), 10.0);
    }

    #[test]
    fn transform_moves_crops_flips_and_fades_rgba() {
        let source = vec![
            255, 0, 0, 255, 0, 255, 0, 255, // top row
            0, 0, 255, 255, 255, 255, 255, 255, // bottom row
        ];
        let mut program = FrameProgram::identity(2.0);
        program.canvas_transform.width = 0.5;
        program.canvas_transform.flip_horizontal = true;
        program.position_x = NumericCurve::constant(0.5);
        program.crop_bottom = NumericCurve::constant(0.25);
        program.opacity = NumericCurve::constant(0.5);
        program.fade_in_seconds = 1.0;
        let sampled = program.sample(0.5).expect("sampled program");
        let output = transform_rgba8_srgb(&source, 2, 2, &sampled).expect("transformed frame");
        assert_eq!(output.len(), source.len());
        assert!(output.chunks_exact(4).any(|pixel| pixel[3] > 0));
        assert!(output.chunks_exact(4).all(|pixel| pixel[3] <= 64));
    }

    #[test]
    fn horizontal_flip_reverses_source_pixels() {
        let source = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let mut program = FrameProgram::identity(1.0);
        program.canvas_transform.flip_horizontal = true;
        let output = transform_rgba8_srgb(
            &source,
            2,
            1,
            &program.sample(0.0).expect("sampled program"),
        )
        .expect("flipped frame");
        assert_eq!(output, vec![0, 255, 0, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn invalid_animated_crop_fails_closed() {
        let mut program = FrameProgram::identity(1.0);
        program.crop_left = NumericCurve::constant(0.6);
        program.crop_right = NumericCurve::constant(0.4);
        assert_eq!(program.sample(0.5), Err(FrameProgramError::InvalidCrop));
    }
}
