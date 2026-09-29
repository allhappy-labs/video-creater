use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct CubeLut {
    size: usize,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
    values: Vec<[f32; 3]>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LutError {
    #[error("LUT size must be between 2 and 65")]
    InvalidSize,
    #[error("LUT domain must be finite and increasing")]
    InvalidDomain,
    #[error("LUT contains an invalid numeric row at line {0}")]
    InvalidRow(usize),
    #[error("LUT expected {expected} entries, got {actual}")]
    WrongEntryCount { expected: usize, actual: usize },
    #[error("LUT is missing LUT_3D_SIZE")]
    MissingSize,
}

impl CubeLut {
    pub fn parse(source: &str) -> Result<Self, LutError> {
        let mut size = None;
        let mut domain_min = [0.0, 0.0, 0.0];
        let mut domain_max = [1.0, 1.0, 1.0];
        let mut values = Vec::new();
        for (line_index, raw_line) in source.lines().enumerate() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with("TITLE") {
                continue;
            }
            let mut fields = line.split_whitespace();
            let first = fields.next().unwrap_or("");
            match first {
                "LUT_3D_SIZE" => {
                    let parsed = fields
                        .next()
                        .and_then(|value| value.parse::<usize>().ok())
                        .ok_or(LutError::InvalidSize)?;
                    if !(2..=65).contains(&parsed) || fields.next().is_some() {
                        return Err(LutError::InvalidSize);
                    }
                    size = Some(parsed);
                }
                "DOMAIN_MIN" => {
                    domain_min = parse_triplet(fields, line_index + 1)?;
                }
                "DOMAIN_MAX" => {
                    domain_max = parse_triplet(fields, line_index + 1)?;
                }
                _ => {
                    let mut row = vec![first];
                    row.extend(fields);
                    if row.len() != 3 {
                        return Err(LutError::InvalidRow(line_index + 1));
                    }
                    let parsed = row
                        .iter()
                        .map(|value| value.parse::<f32>().ok())
                        .collect::<Option<Vec<_>>>()
                        .ok_or(LutError::InvalidRow(line_index + 1))?;
                    if parsed.iter().any(|value| !value.is_finite()) {
                        return Err(LutError::InvalidRow(line_index + 1));
                    }
                    values.push([parsed[0], parsed[1], parsed[2]]);
                }
            }
        }
        let size = size.ok_or(LutError::MissingSize)?;
        if (0..3).any(|channel| {
            !domain_min[channel].is_finite()
                || !domain_max[channel].is_finite()
                || domain_max[channel] <= domain_min[channel]
        }) {
            return Err(LutError::InvalidDomain);
        }
        let expected = size * size * size;
        if values.len() != expected {
            return Err(LutError::WrongEntryCount {
                expected,
                actual: values.len(),
            });
        }
        Ok(Self {
            size,
            domain_min,
            domain_max,
            values,
        })
    }

    pub fn apply_rgb(&self, rgb: [f32; 3]) -> [f32; 3] {
        let coordinate = |channel: usize| {
            ((rgb[channel] - self.domain_min[channel])
                / (self.domain_max[channel] - self.domain_min[channel]))
                .clamp(0.0, 1.0)
                * (self.size - 1) as f32
        };
        let position = [coordinate(0), coordinate(1), coordinate(2)];
        let low = position.map(|value| value.floor() as usize);
        let high = position.map(|value| (value.ceil() as usize).min(self.size - 1));
        let fraction = [
            position[0] - low[0] as f32,
            position[1] - low[1] as f32,
            position[2] - low[2] as f32,
        ];
        let sample = |red: usize, green: usize, blue: usize| {
            self.values[red + self.size * green + self.size * self.size * blue]
        };
        let lerp = |left: [f32; 3], right: [f32; 3], amount: f32| {
            [
                left[0] + (right[0] - left[0]) * amount,
                left[1] + (right[1] - left[1]) * amount,
                left[2] + (right[2] - left[2]) * amount,
            ]
        };
        let c000 = sample(low[0], low[1], low[2]);
        let c100 = sample(high[0], low[1], low[2]);
        let c010 = sample(low[0], high[1], low[2]);
        let c110 = sample(high[0], high[1], low[2]);
        let c001 = sample(low[0], low[1], high[2]);
        let c101 = sample(high[0], low[1], high[2]);
        let c011 = sample(low[0], high[1], high[2]);
        let c111 = sample(high[0], high[1], high[2]);
        let low_blue = lerp(
            lerp(c000, c100, fraction[0]),
            lerp(c010, c110, fraction[0]),
            fraction[1],
        );
        let high_blue = lerp(
            lerp(c001, c101, fraction[0]),
            lerp(c011, c111, fraction[0]),
            fraction[1],
        );
        lerp(low_blue, high_blue, fraction[2])
    }

    pub fn to_canonical_cube(&self) -> String {
        let mut output = format!("LUT_3D_SIZE {}\n", self.size);
        output.push_str(&format!(
            "DOMAIN_MIN {} {} {}\n",
            canonical_float(self.domain_min[0]),
            canonical_float(self.domain_min[1]),
            canonical_float(self.domain_min[2])
        ));
        output.push_str(&format!(
            "DOMAIN_MAX {} {} {}\n",
            canonical_float(self.domain_max[0]),
            canonical_float(self.domain_max[1]),
            canonical_float(self.domain_max[2])
        ));
        for value in &self.values {
            output.push_str(&format!(
                "{} {} {}\n",
                canonical_float(value[0]),
                canonical_float(value[1]),
                canonical_float(value[2])
            ));
        }
        output
    }

    pub fn apply_rgba8_srgb(&self, rgba: [u8; 4]) -> [u8; 4] {
        self.apply_rgba8_srgb_strength(rgba, 1.0)
    }

    pub fn apply_rgba8_srgb_strength(&self, rgba: [u8; 4], strength: f32) -> [u8; 4] {
        let transformed = self.apply_rgb([
            f32::from(rgba[0]) / 255.0,
            f32::from(rgba[1]) / 255.0,
            f32::from(rgba[2]) / 255.0,
        ]);
        let strength = strength.clamp(0.0, 1.0);
        let original = [
            f32::from(rgba[0]) / 255.0,
            f32::from(rgba[1]) / 255.0,
            f32::from(rgba[2]) / 255.0,
        ];
        let mixed = [
            original[0] + (transformed[0] - original[0]) * strength,
            original[1] + (transformed[1] - original[1]) * strength,
            original[2] + (transformed[2] - original[2]) * strength,
        ];
        [
            (mixed[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (mixed[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (mixed[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            rgba[3],
        ]
    }
}

fn canonical_float(value: f32) -> String {
    let mut value = if value == 0.0 { 0.0 } else { value };
    if !value.is_finite() {
        value = 0.0;
    }
    let mut formatted = format!("{value:.9}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}

fn parse_triplet<'a>(
    fields: impl Iterator<Item = &'a str>,
    line: usize,
) -> Result<[f32; 3], LutError> {
    let values = fields
        .map(|value| value.parse::<f32>().ok())
        .collect::<Option<Vec<_>>>()
        .ok_or(LutError::InvalidRow(line))?;
    if values.len() != 3 || values.iter().any(|value| !value.is_finite()) {
        return Err(LutError::InvalidRow(line));
    }
    Ok([values[0], values[1], values[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: &str =
        "LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";

    #[test]
    fn identity_cube_preserves_interpolated_values_and_alpha() {
        let lut = CubeLut::parse(IDENTITY).expect("identity LUT");
        assert_eq!(lut.apply_rgba8_srgb([64, 128, 192, 77]), [64, 128, 192, 77]);
    }

    #[test]
    fn trilinear_interpolation_applies_known_inversion() {
        let invert = "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n";
        let lut = CubeLut::parse(invert).expect("invert LUT");
        assert_eq!(
            lut.apply_rgba8_srgb([64, 128, 192, 255]),
            [191, 127, 63, 255]
        );
    }

    #[test]
    fn strength_blends_in_srgb_and_preserves_alpha() {
        let invert = "LUT_3D_SIZE 2\n1 1 1\n0 1 1\n1 0 1\n0 0 1\n1 1 0\n0 1 0\n1 0 0\n0 0 0\n";
        let lut = CubeLut::parse(invert).expect("invert LUT");
        assert_eq!(
            lut.apply_rgba8_srgb_strength([64, 128, 192, 77], 0.5),
            [128, 128, 128, 77]
        );
    }

    #[test]
    fn rejects_wrong_entry_count() {
        assert_eq!(
            CubeLut::parse("LUT_3D_SIZE 2\n0 0 0\n"),
            Err(LutError::WrongEntryCount {
                expected: 8,
                actual: 1
            })
        );
    }

    #[test]
    fn canonical_serialization_ignores_comments_and_numeric_spelling() {
        let first = CubeLut::parse(IDENTITY).expect("identity LUT");
        let second = CubeLut::parse(
            "# equivalent\nLUT_3D_SIZE 2\n0.0 0 0\n1.000 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n",
        )
        .expect("equivalent LUT");
        assert_eq!(first.to_canonical_cube(), second.to_canonical_cube());
        assert_eq!(
            CubeLut::parse(&first.to_canonical_cube()).expect("canonical LUT"),
            first
        );
    }
}
