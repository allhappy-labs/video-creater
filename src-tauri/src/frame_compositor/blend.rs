#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Source,
    Over,
    Add,
    Darken,
    Multiply,
    ColorBurn,
    Lighten,
    Screen,
    ColorDodge,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

#[derive(Debug, Clone, Copy)]
struct LinearPremultiplied {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

pub fn composite_rgba8_srgb(backdrop: [u8; 4], source: [u8; 4], mode: BlendMode) -> [u8; 4] {
    let backdrop = from_rgba8(backdrop);
    let source = from_rgba8(source);
    to_rgba8(composite(backdrop, source, mode))
}

fn composite(
    backdrop: LinearPremultiplied,
    source: LinearPremultiplied,
    mode: BlendMode,
) -> LinearPremultiplied {
    if mode == BlendMode::Source {
        return source;
    }
    if mode == BlendMode::Add {
        return LinearPremultiplied {
            red: (backdrop.red + source.red).min(1.0),
            green: (backdrop.green + source.green).min(1.0),
            blue: (backdrop.blue + source.blue).min(1.0),
            alpha: (backdrop.alpha + source.alpha).min(1.0),
        };
    }
    let output_alpha = source.alpha + backdrop.alpha * (1.0 - source.alpha);
    if mode == BlendMode::Over {
        return LinearPremultiplied {
            red: source.red + backdrop.red * (1.0 - source.alpha),
            green: source.green + backdrop.green * (1.0 - source.alpha),
            blue: source.blue + backdrop.blue * (1.0 - source.alpha),
            alpha: output_alpha,
        };
    }

    let source_straight = unpremultiplied(source);
    let backdrop_straight = unpremultiplied(backdrop);
    let blended = blend_straight(backdrop_straight, source_straight, mode);
    let channel = |channel_index: usize| {
        let backdrop_channel = backdrop_straight[channel_index];
        let source_channel = source_straight[channel_index];
        (1.0 - source.alpha) * backdrop_channel * backdrop.alpha
            + (1.0 - backdrop.alpha) * source_channel * source.alpha
            + source.alpha * backdrop.alpha * blended[channel_index]
    };
    LinearPremultiplied {
        red: channel(0),
        green: channel(1),
        blue: channel(2),
        alpha: output_alpha,
    }
}

fn blend_straight(backdrop: [f32; 3], source: [f32; 3], mode: BlendMode) -> [f32; 3] {
    match mode {
        BlendMode::Hue => set_luminosity(
            set_saturation(source, saturation(backdrop)),
            luminosity(backdrop),
        ),
        BlendMode::Saturation => set_luminosity(
            set_saturation(backdrop, saturation(source)),
            luminosity(backdrop),
        ),
        BlendMode::Color => set_luminosity(source, luminosity(backdrop)),
        BlendMode::Luminosity => set_luminosity(backdrop, luminosity(source)),
        _ => std::array::from_fn(|index| blend_channel(backdrop[index], source[index], mode)),
    }
}

fn blend_channel(backdrop: f32, source: f32, mode: BlendMode) -> f32 {
    match mode {
        BlendMode::Darken => backdrop.min(source),
        BlendMode::Multiply => backdrop * source,
        BlendMode::ColorBurn => {
            if source <= f32::EPSILON {
                0.0
            } else {
                1.0 - ((1.0 - backdrop) / source).min(1.0)
            }
        }
        BlendMode::Lighten => backdrop.max(source),
        BlendMode::Screen => backdrop + source - backdrop * source,
        BlendMode::ColorDodge => {
            if source >= 1.0 - f32::EPSILON {
                1.0
            } else {
                (backdrop / (1.0 - source)).min(1.0)
            }
        }
        BlendMode::Overlay => hard_light(source, backdrop),
        BlendMode::SoftLight => {
            if source <= 0.5 {
                backdrop - (1.0 - 2.0 * source) * backdrop * (1.0 - backdrop)
            } else {
                let d = if backdrop <= 0.25 {
                    ((16.0 * backdrop - 12.0) * backdrop + 4.0) * backdrop
                } else {
                    backdrop.sqrt()
                };
                backdrop + (2.0 * source - 1.0) * (d - backdrop)
            }
        }
        BlendMode::HardLight => hard_light(backdrop, source),
        BlendMode::Difference => (backdrop - source).abs(),
        BlendMode::Exclusion => backdrop + source - 2.0 * backdrop * source,
        _ => unreachable!("special and Porter-Duff modes are handled before per-channel blending"),
    }
}

fn hard_light(backdrop: f32, source: f32) -> f32 {
    if source <= 0.5 {
        2.0 * backdrop * source
    } else {
        1.0 - 2.0 * (1.0 - backdrop) * (1.0 - source)
    }
}

fn luminosity(color: [f32; 3]) -> f32 {
    0.3 * color[0] + 0.59 * color[1] + 0.11 * color[2]
}

fn saturation(color: [f32; 3]) -> f32 {
    color.iter().copied().fold(f32::MIN, f32::max) - color.iter().copied().fold(f32::MAX, f32::min)
}

fn set_luminosity(color: [f32; 3], target: f32) -> [f32; 3] {
    let delta = target - luminosity(color);
    clip_color(color.map(|channel| channel + delta))
}

fn clip_color(mut color: [f32; 3]) -> [f32; 3] {
    let luminosity = luminosity(color);
    let minimum = color.iter().copied().fold(f32::MAX, f32::min);
    let maximum = color.iter().copied().fold(f32::MIN, f32::max);
    if minimum < 0.0 {
        let denominator = luminosity - minimum;
        if denominator.abs() > f32::EPSILON {
            color =
                color.map(|channel| luminosity + (channel - luminosity) * luminosity / denominator);
        }
    }
    if maximum > 1.0 {
        let denominator = maximum - luminosity;
        if denominator.abs() > f32::EPSILON {
            color = color.map(|channel| {
                luminosity + (channel - luminosity) * (1.0 - luminosity) / denominator
            });
        }
    }
    color.map(|channel| channel.clamp(0.0, 1.0))
}

fn set_saturation(color: [f32; 3], target: f32) -> [f32; 3] {
    let mut indices = [0_usize, 1, 2];
    indices.sort_by(|left, right| color[*left].total_cmp(&color[*right]));
    let (minimum, middle, maximum) = (indices[0], indices[1], indices[2]);
    let mut result = [0.0; 3];
    if color[maximum] > color[minimum] {
        result[middle] =
            (color[middle] - color[minimum]) * target / (color[maximum] - color[minimum]);
        result[maximum] = target;
    }
    result[minimum] = 0.0;
    result
}

fn from_rgba8(rgba: [u8; 4]) -> LinearPremultiplied {
    let alpha = f32::from(rgba[3]) / 255.0;
    LinearPremultiplied {
        red: srgb_to_linear(f32::from(rgba[0]) / 255.0) * alpha,
        green: srgb_to_linear(f32::from(rgba[1]) / 255.0) * alpha,
        blue: srgb_to_linear(f32::from(rgba[2]) / 255.0) * alpha,
        alpha,
    }
}

fn to_rgba8(pixel: LinearPremultiplied) -> [u8; 4] {
    if pixel.alpha <= f32::EPSILON {
        return [0, 0, 0, 0];
    }
    let channel =
        |value: f32| (linear_to_srgb((value / pixel.alpha).clamp(0.0, 1.0)) * 255.0).round() as u8;
    [
        channel(pixel.red),
        channel(pixel.green),
        channel(pixel.blue),
        (pixel.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn unpremultiplied(pixel: LinearPremultiplied) -> [f32; 3] {
    if pixel.alpha <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [
            pixel.red / pixel.alpha,
            pixel.green / pixel.alpha,
            pixel.blue / pixel.alpha,
        ]
    }
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
    fn transparent_source_preserves_backdrop_for_all_destination_blends() {
        let backdrop = [20, 80, 160, 200];
        for mode in [
            BlendMode::Over,
            BlendMode::Darken,
            BlendMode::Multiply,
            BlendMode::ColorBurn,
            BlendMode::Lighten,
            BlendMode::Screen,
            BlendMode::ColorDodge,
            BlendMode::Overlay,
            BlendMode::SoftLight,
            BlendMode::HardLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ] {
            assert_eq!(
                composite_rgba8_srgb(backdrop, [255, 0, 0, 0], mode),
                backdrop
            );
        }
    }

    fn assert_color_close(actual: [f32; 3], expected: [f32; 3]) {
        for (channel, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
            assert!(
                (actual - expected).abs() <= 0.000_01,
                "channel={channel} actual={actual} expected={expected}"
            );
        }
    }

    #[test]
    fn separable_palmier_blends_match_reference_vectors() {
        let backdrop = [0.25, 0.5, 0.75];
        let source = [0.8, 0.4, 0.2];
        for (mode, expected) in [
            (BlendMode::Darken, [0.25, 0.4, 0.2]),
            (BlendMode::Multiply, [0.2, 0.2, 0.15]),
            (BlendMode::ColorBurn, [0.0625, 0.0, 0.0]),
            (BlendMode::Lighten, [0.8, 0.5, 0.75]),
            (BlendMode::Screen, [0.85, 0.7, 0.8]),
            (BlendMode::ColorDodge, [1.0, 5.0 / 6.0, 0.9375]),
            (BlendMode::Overlay, [0.4, 0.4, 0.6]),
            (BlendMode::HardLight, [0.7, 0.4, 0.3]),
            (BlendMode::Difference, [0.55, 0.1, 0.55]),
            (BlendMode::Exclusion, [0.65, 0.5, 0.65]),
        ] {
            assert_color_close(blend_straight(backdrop, source, mode), expected);
        }
    }

    #[test]
    fn soft_light_matches_both_reference_branches() {
        assert_color_close(
            blend_straight([0.25, 0.5, 0.75], [0.25, 0.75, 0.25], BlendMode::SoftLight),
            [0.15625, 0.6035534, 0.65625],
        );
    }

    #[test]
    fn nonseparable_palmier_blends_preserve_their_reference_components() {
        let backdrop = [0.2, 0.4, 0.6];
        let source = [0.6, 0.5, 0.4];
        let hue = blend_straight(backdrop, source, BlendMode::Hue);
        let saturation_blend = blend_straight(backdrop, source, BlendMode::Saturation);
        let color = blend_straight(backdrop, source, BlendMode::Color);
        let luminosity_blend = blend_straight(backdrop, source, BlendMode::Luminosity);

        assert!((luminosity(hue) - luminosity(backdrop)).abs() <= 0.000_01);
        assert!((saturation(hue) - saturation(backdrop)).abs() <= 0.000_01);
        assert!((luminosity(saturation_blend) - luminosity(backdrop)).abs() <= 0.000_01);
        assert!((saturation(saturation_blend) - saturation(source)).abs() <= 0.000_01);
        assert!((luminosity(color) - luminosity(backdrop)).abs() <= 0.000_01);
        assert!((saturation(color) - saturation(source)).abs() <= 0.000_01);
        assert!((luminosity(luminosity_blend) - luminosity(source)).abs() <= 0.000_01);
        assert!((saturation(luminosity_blend) - saturation(backdrop)).abs() <= 0.000_01);
    }

    #[test]
    fn opaque_multiply_screen_and_overlay_match_linear_light_vectors() {
        let backdrop = [128, 128, 128, 255];
        let source = [128, 64, 192, 255];
        assert_eq!(
            composite_rgba8_srgb(backdrop, source, BlendMode::Multiply),
            [61, 27, 95, 255]
        );
        assert_eq!(
            composite_rgba8_srgb(backdrop, source, BlendMode::Screen),
            [167, 138, 208, 255]
        );
        assert_eq!(
            composite_rgba8_srgb(backdrop, source, BlendMode::Overlay),
            [86, 41, 131, 255]
        );
    }

    #[test]
    fn source_replaces_and_add_saturates() {
        assert_eq!(
            composite_rgba8_srgb([200, 100, 50, 255], [100, 200, 250, 128], BlendMode::Source),
            [100, 200, 250, 128]
        );
        assert_eq!(
            composite_rgba8_srgb([255, 255, 255, 255], [255, 255, 255, 255], BlendMode::Add),
            [255, 255, 255, 255]
        );
    }

    #[test]
    fn mixed_alpha_over_matches_the_linear_premultiplied_contract() {
        assert_eq!(
            composite_rgba8_srgb([20, 80, 160, 128], [200, 40, 100, 128], BlendMode::Over),
            [167, 57, 124, 192]
        );
    }
}
