//! `video-creater-audio-enhance` for non-Apple desktops.
//!
//! Implements the same command-line contract as the macOS Core ML helper:
//! `--input IN.wav --output OUT.wav --model-directory DIR --strength 0..1`.
//! The input must be RIFF PCM16 at 48 kHz with any channel count. Each channel is
//! enhanced independently with DeepFilterNet3 and mixed as `dry * (1 - s) + wet * s`.
//! The output keeps the channel count, frame count, and PCM16 48 kHz format. Failures
//! are reported on stderr as `audio enhancement failed: ...` with exit status 2.

use df::tract::{DfParams, DfTract, RuntimeParams};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use ndarray::{Array2, Axis};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SAMPLE_RATE: u32 = 48_000;
/// Identifier of the embedded model, recorded next to the model directory contract.
const MODEL_ID: &str = "deepfilternet3-onnx-tract";
const MODEL_ARCHIVE_SHA256: &str =
    "c94d91f70911001c946e0fabb4aa9adc37045f45a03b56008cb0c8244cb63616";
const LIBDF_REVISION: &str = "d375b2d8309e0935d165700c91da9de862a99c31";

#[derive(Debug)]
struct Failure(String);

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn failure(message: impl Into<String>) -> Failure {
    Failure(message.into())
}

#[derive(Debug, Clone, PartialEq)]
struct Arguments {
    input: PathBuf,
    output: PathBuf,
    model_directory: PathBuf,
    strength: f32,
}

fn parse_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Arguments, Failure> {
    let mut input = None;
    let mut output = None;
    let mut model_directory = None;
    let mut strength = None;
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| failure(format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--input" => input = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            "--model-directory" => model_directory = Some(PathBuf::from(value)),
            "--strength" => {
                let parsed = value
                    .parse::<f32>()
                    .map_err(|_| failure("strength must be a number between 0 and 1"))?;
                if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
                    return Err(failure("strength must be a number between 0 and 1"));
                }
                strength = Some(parsed);
            }
            _ => return Err(failure(format!("unknown argument {flag}"))),
        }
    }
    Ok(Arguments {
        input: input.ok_or_else(|| failure("--input is required"))?,
        output: output.ok_or_else(|| failure("--output is required"))?,
        model_directory: model_directory.ok_or_else(|| failure("--model-directory is required"))?,
        strength: strength.ok_or_else(|| failure("--strength is required"))?,
    })
}

/// Planar PCM samples in the range [-1, 1], one vector per channel.
#[derive(Debug, Clone, PartialEq)]
struct Pcm16Wave {
    sample_rate: u32,
    channels: Vec<Vec<f32>>,
}

fn read_pcm16_wave(path: &Path) -> Result<Pcm16Wave, Failure> {
    let mut reader = WavReader::open(path)
        .map_err(|error| failure(format!("input must be a RIFF/WAVE file: {error}")))?;
    let spec = reader.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 || spec.channels == 0 {
        return Err(failure("input must be interleaved PCM16 WAV"));
    }
    let channel_count = usize::from(spec.channels);
    let mut channels = vec![Vec::with_capacity(reader.duration() as usize); channel_count];
    for (index, sample) in reader.samples::<i16>().enumerate() {
        let sample = sample.map_err(|error| failure(format!("input PCM is invalid: {error}")))?;
        channels[index % channel_count].push(f32::from(sample) / 32768.0);
    }
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    for channel in &mut channels {
        channel.truncate(frames);
    }
    Ok(Pcm16Wave {
        sample_rate: spec.sample_rate,
        channels,
    })
}

fn write_pcm16_wave(path: &Path, wave: &Pcm16Wave) -> Result<(), Failure> {
    let frames = wave.channels.first().map(Vec::len).unwrap_or(0);
    if frames == 0 || wave.channels.iter().any(|channel| channel.len() != frames) {
        return Err(failure("enhancer returned invalid channels"));
    }
    let channel_count = u16::try_from(wave.channels.len())
        .map_err(|_| failure("enhancer returned too many channels"))?;
    let spec = WavSpec {
        channels: channel_count,
        sample_rate: wave.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output.wav");
    let staging = path.with_file_name(format!(".{file_name}.{}.partial", std::process::id()));
    let result = (|| {
        let mut writer = WavWriter::create(&staging, spec)
            .map_err(|error| failure(format!("output WAV could not be created: {error}")))?;
        for frame in 0..frames {
            for channel in &wave.channels {
                let value = (channel[frame].clamp(-1.0, 1.0) * 32767.0).round() as i16;
                writer
                    .write_sample(value)
                    .map_err(|error| failure(format!("output WAV write failed: {error}")))?;
            }
        }
        writer
            .finalize()
            .map_err(|error| failure(format!("output WAV finalize failed: {error}")))?;
        fs::rename(&staging, path)
            .map_err(|error| failure(format!("output WAV could not be published: {error}")))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

/// DeepFilterNet3 wrapper that enhances one channel at a time with fresh model state.
struct Enhancer {
    params: DfParams,
}

impl Enhancer {
    fn embedded() -> Result<Self, Failure> {
        let params = std::panic::catch_unwind(DfParams::default)
            .map_err(|_| failure("embedded DeepFilterNet3 model could not be loaded"))?;
        Ok(Self { params })
    }

    fn runtime_params() -> RuntimeParams {
        // Matches the reference `deep-filter` CLI defaults of the pinned libDF revision.
        RuntimeParams::default_with_ch(1)
            .with_atten_lim(100.0)
            .with_thresholds(-15.0, 35.0, 35.0)
    }

    /// Returns a delay-compensated enhanced signal with exactly `dry.len()` samples.
    fn enhance_channel(&self, dry: &[f32]) -> Result<Vec<f32>, Failure> {
        let mut model = DfTract::new(self.params.clone(), &Self::runtime_params())
            .map_err(|error| failure(format!("DeepFilterNet3 model init failed: {error}")))?;
        if model.sr != SAMPLE_RATE as usize {
            return Err(failure(format!(
                "DeepFilterNet3 model sample rate is {}, expected {SAMPLE_RATE}",
                model.sr
            )));
        }
        let hop = model.hop_size;
        let delay = model.fft_size - model.hop_size + model.lookahead * model.hop_size;
        let padded_len = (dry.len() + delay).div_ceil(hop) * hop;
        let mut noisy = Array2::<f32>::zeros((1, padded_len));
        noisy
            .row_mut(0)
            .as_slice_mut()
            .ok_or_else(|| failure("noisy buffer is not contiguous"))?[..dry.len()]
            .copy_from_slice(dry);
        let mut enhanced = Array2::<f32>::zeros((1, padded_len));
        for (noisy_hop, enhanced_hop) in noisy
            .axis_chunks_iter(Axis(1), hop)
            .zip(enhanced.axis_chunks_iter_mut(Axis(1), hop))
        {
            model
                .process(noisy_hop, enhanced_hop)
                .map_err(|error| failure(format!("DeepFilterNet3 inference failed: {error}")))?;
        }
        let enhanced = enhanced.row(0);
        let wet = enhanced
            .as_slice()
            .ok_or_else(|| failure("enhanced buffer is not contiguous"))?;
        Ok(wet[delay..delay + dry.len()].to_vec())
    }
}

fn mix(dry: &[f32], wet: &[f32], strength: f32) -> Vec<f32> {
    dry.iter()
        .zip(wet)
        .map(|(dry, wet)| dry * (1.0 - strength) + wet * strength)
        .collect()
}

fn enhance_wave(
    enhancer: &Enhancer,
    input: &Pcm16Wave,
    strength: f32,
) -> Result<Pcm16Wave, Failure> {
    if input.sample_rate != SAMPLE_RATE {
        return Err(failure(format!(
            "input sample rate must be {SAMPLE_RATE} Hz"
        )));
    }
    let mut channels = Vec::with_capacity(input.channels.len());
    for dry in &input.channels {
        let wet = if dry.is_empty() || strength == 0.0 {
            dry.clone()
        } else {
            enhancer.enhance_channel(dry)?
        };
        channels.push(mix(dry, &wet, strength));
    }
    Ok(Pcm16Wave {
        sample_rate: SAMPLE_RATE,
        channels,
    })
}

fn ensure_model_directory(directory: &Path) -> Result<(), Failure> {
    // The DeepFilterNet3 weights are compiled into this helper, so the model directory
    // only records which embedded model produced the output; nothing is downloaded.
    let model_directory = directory.join("models").join(MODEL_ID);
    fs::create_dir_all(&model_directory)
        .map_err(|error| failure(format!("model directory could not be created: {error}")))?;
    let record = model_directory.join("embedded-model.json");
    let contents = format!(
        "{{\n  \"model\": \"DeepFilterNet3\",\n  \"source\": \"https://github.com/Rikorose/DeepFilterNet/blob/{LIBDF_REVISION}/models/DeepFilterNet3_onnx.tar.gz\",\n  \"sha256\": \"{MODEL_ARCHIVE_SHA256}\",\n  \"license\": \"MIT OR Apache-2.0\",\n  \"embedded\": true\n}}\n"
    );
    if fs::read_to_string(&record).ok().as_deref() != Some(contents.as_str()) {
        fs::write(&record, contents)
            .map_err(|error| failure(format!("model record could not be written: {error}")))?;
    }
    Ok(())
}

fn run(arguments: Arguments) -> Result<(), Failure> {
    let input = read_pcm16_wave(&arguments.input)?;
    if input.sample_rate != SAMPLE_RATE {
        return Err(failure(format!(
            "input sample rate must be {SAMPLE_RATE} Hz"
        )));
    }
    ensure_model_directory(&arguments.model_directory)?;
    let enhancer = Enhancer::embedded()?;
    let output = enhance_wave(&enhancer, &input, arguments.strength)?;
    write_pcm16_wave(&arguments.output, &output)
}

fn main() -> ExitCode {
    let result = parse_arguments(std::env::args().skip(1)).and_then(run);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("audio enhancement failed: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn lcg_noise(seed: &mut u64) -> f32 {
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        ((*seed >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }

    /// Voiced-speech-like bursts (harmonic series with a moving pitch and syllable
    /// envelope) between noise-only gaps.
    fn noisy_speech_like(frames: usize, seed: u64, noise_level: f32) -> (Vec<f32>, Vec<bool>) {
        let mut seed = seed;
        let mut samples = Vec::with_capacity(frames);
        let mut voiced = Vec::with_capacity(frames);
        let mut phase = 0.0_f32;
        for index in 0..frames {
            let seconds = index as f32 / SAMPLE_RATE as f32;
            let cycle = seconds % 1.0;
            let active = (0.2..0.7).contains(&cycle);
            let pitch = 140.0 + 30.0 * (seconds * 3.0).sin();
            phase += std::f32::consts::TAU * pitch / SAMPLE_RATE as f32;
            let envelope = if active {
                ((cycle - 0.2) / 0.5 * std::f32::consts::PI).sin()
                    * (0.6 + 0.4 * (seconds * 11.0).sin().abs())
            } else {
                0.0
            };
            let harmonics = (1..=12)
                .map(|harmonic| (phase * harmonic as f32).sin() / harmonic as f32)
                .sum::<f32>();
            samples.push(harmonics * envelope * 0.25 + lcg_noise(&mut seed) * noise_level);
            voiced.push(active);
        }
        (samples, voiced)
    }

    fn rms(values: impl Iterator<Item = f32>) -> f64 {
        let (sum, count) = values.fold((0.0_f64, 0usize), |(sum, count), value| {
            (sum + f64::from(value).powi(2), count + 1)
        });
        (sum / count.max(1) as f64).sqrt()
    }

    #[test]
    fn parses_the_shared_cli_contract_and_rejects_invalid_strength() {
        let parsed = parse_arguments(arguments(&[
            "--input",
            "in.wav",
            "--output",
            "out.wav",
            "--model-directory",
            "models",
            "--strength",
            "0.6",
        ]))
        .expect("valid arguments");
        assert_eq!(parsed.input, PathBuf::from("in.wav"));
        assert_eq!(parsed.output, PathBuf::from("out.wav"));
        assert_eq!(parsed.model_directory, PathBuf::from("models"));
        assert!((parsed.strength - 0.6).abs() < f32::EPSILON);
        for strength in ["1.5", "-0.1", "NaN", "loud"] {
            assert!(parse_arguments(arguments(&[
                "--input",
                "a",
                "--output",
                "b",
                "--model-directory",
                "c",
                "--strength",
                strength,
            ]))
            .is_err());
        }
        assert!(parse_arguments(arguments(&["--input", "a"])).is_err());
    }

    #[test]
    fn rejects_non_pcm16_and_non_48k_inputs() {
        let directory = tempfile::tempdir().expect("tempdir");
        let float_path = directory.path().join("float.wav");
        let mut writer = WavWriter::create(
            &float_path,
            WavSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 32,
                sample_format: SampleFormat::Float,
            },
        )
        .expect("float writer");
        writer.write_sample(0.25_f32).expect("sample");
        writer.finalize().expect("finalize");
        assert!(read_pcm16_wave(&float_path).is_err());

        let low_rate = directory.path().join("low-rate.wav");
        write_pcm16_wave(
            &low_rate,
            &Pcm16Wave {
                sample_rate: 16_000,
                channels: vec![vec![0.1; 1600]],
            },
        )
        .expect("write low-rate");
        let error = run(Arguments {
            input: low_rate,
            output: directory.path().join("out.wav"),
            model_directory: directory.path().join("models"),
            strength: 1.0,
        })
        .expect_err("48 kHz is required");
        assert!(error.0.contains("48000"), "{error}");
    }

    // tract-core 0.21.4 runs `check_compact` only under `debug_assertions`, and that check
    // rejects the embedded DeepFilterNet3 graph after codegen ("duplicate name
    // /convt3/Conv.bias"). Release builds skip the check and run the model correctly; the
    // helper is always shipped with the release profile.
    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "tract-core's debug-only graph check rejects the DeepFilterNet3 model; run with --release"
    )]
    fn deepfilternet3_suppresses_noise_and_preserves_frames_and_channels() {
        let directory = tempfile::tempdir().expect("tempdir");
        let frames = SAMPLE_RATE as usize * 3 + 123;
        let (left, voiced) = noisy_speech_like(frames, 7, 0.05);
        let (right, _) = noisy_speech_like(frames, 11, 0.02);
        let input_path = directory.path().join("input.wav");
        let output_path = directory.path().join("output.wav");
        write_pcm16_wave(
            &input_path,
            &Pcm16Wave {
                sample_rate: SAMPLE_RATE,
                channels: vec![left.clone(), right.clone()],
            },
        )
        .expect("write input");

        run(Arguments {
            input: input_path.clone(),
            output: output_path.clone(),
            model_directory: directory.path().join("models"),
            strength: 1.0,
        })
        .expect("enhance");

        let output = read_pcm16_wave(&output_path).expect("read output");
        assert_eq!(output.sample_rate, SAMPLE_RATE);
        assert_eq!(output.channels.len(), 2);
        assert!(output
            .channels
            .iter()
            .all(|channel| channel.len() == frames));
        assert!(directory
            .path()
            .join("models/models")
            .join(MODEL_ID)
            .join("embedded-model.json")
            .is_file());

        // Noise-only gaps after the model has warmed up (skip the first second).
        let gap = |samples: &[f32]| {
            rms(samples
                .iter()
                .zip(&voiced)
                .enumerate()
                .filter(|(index, (_, voiced))| *index > SAMPLE_RATE as usize && !**voiced)
                .map(|(_, (sample, _))| *sample))
        };
        let voiced_rms = |samples: &[f32]| {
            rms(samples
                .iter()
                .zip(&voiced)
                .filter(|(_, voiced)| **voiced)
                .map(|(sample, _)| *sample))
        };
        let before = gap(&left);
        let after = gap(&output.channels[0]);
        assert!(
            after < before * 0.25,
            "noise floor should drop by at least 12 dB: before {before}, after {after}"
        );
        assert!(
            voiced_rms(&output.channels[0]) > voiced_rms(&left) * 0.3,
            "voiced content should survive enhancement"
        );
    }

    #[test]
    fn zero_strength_returns_the_dry_signal() {
        let directory = tempfile::tempdir().expect("tempdir");
        let (samples, _) = noisy_speech_like(4_800, 3, 0.05);
        let input_path = directory.path().join("input.wav");
        let output_path = directory.path().join("output.wav");
        let input = Pcm16Wave {
            sample_rate: SAMPLE_RATE,
            channels: vec![samples],
        };
        write_pcm16_wave(&input_path, &input).expect("write input");
        run(Arguments {
            input: input_path.clone(),
            output: output_path.clone(),
            model_directory: directory.path().join("models"),
            strength: 0.0,
        })
        .expect("dry run");
        assert_eq!(
            fs::read(&input_path).expect("input bytes"),
            fs::read(&output_path).expect("output bytes")
        );
    }

    #[test]
    fn wet_dry_mix_is_linear() {
        assert_eq!(mix(&[1.0, 0.0], &[0.0, 1.0], 0.25), vec![0.75, 0.25]);
    }
}
