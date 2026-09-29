//! `video-creater-semantic-encoder` for non-Apple desktops.
//!
//! Speaks the same protocol as the macOS Core ML helper:
//! `video-creater-semantic-encoder <prepare|serve> --model-dir=<path>`.
//! `serve` reads NDJSON requests from stdin, one per line:
//! `{"id":1,"type":"text","text":"..."}` or `{"id":2,"type":"image","imageBase64":"..."}`,
//! and writes `{"id":1,"embedding":[...]}` or `{"id":1,"error":"..."}` lines to stdout.
//!
//! The model is the SigLIP 2 base patch16/256 ONNX export (text and vision towers) run with
//! ONNX Runtime. Text is lowercased, tokenized with the Gemma tokenizer (EOS appended),
//! truncated/padded to the context length with pad id 0. Images are composited over black,
//! resized to `imageSize` x `imageSize` with bilinear filtering, converted to RGB, and
//! normalized with mean 0.5 / std 0.5. Embeddings are L2-normalized.

use base64::Engine;
use image::imageops::FilterType;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DEFAULT_IMAGE_ENCODER: &str = "vision_model_fp16.onnx";
const DEFAULT_TEXT_ENCODER: &str = "text_model_quantized.onnx";
const DEFAULT_TOKENIZER: &str = "tokenizer.json";
const PAD_TOKEN_ID: i64 = 0;

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

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ModelSpec {
    model: String,
    embedding_dim: usize,
    image_size: u32,
    context_length: usize,
    #[serde(default = "default_image_encoder")]
    image_encoder: String,
    #[serde(default = "default_text_encoder")]
    text_encoder: String,
    #[serde(default = "default_tokenizer")]
    tokenizer: String,
}

fn default_image_encoder() -> String {
    DEFAULT_IMAGE_ENCODER.to_string()
}
fn default_text_encoder() -> String {
    DEFAULT_TEXT_ENCODER.to_string()
}
fn default_tokenizer() -> String {
    DEFAULT_TOKENIZER.to_string()
}

impl ModelSpec {
    fn load(model_dir: &Path) -> Result<Self, Failure> {
        let bytes = std::fs::read(model_dir.join("spec.json"))
            .map_err(|error| failure(format!("missing spec.json: {error}")))?;
        let spec: Self = serde_json::from_slice(&bytes)
            .map_err(|error| failure(format!("invalid spec.json: {error}")))?;
        if spec.embedding_dim == 0 || spec.image_size == 0 || spec.context_length == 0 {
            return Err(failure("spec.json dimensions must be positive"));
        }
        for file in [&spec.image_encoder, &spec.text_encoder, &spec.tokenizer] {
            if file.is_empty() || file.contains('/') || file.contains("..") {
                return Err(failure(format!("spec.json file name {file:?} is unsafe")));
            }
        }
        Ok(spec)
    }

    fn require_files(&self, model_dir: &Path) -> Result<(), Failure> {
        for file in [&self.image_encoder, &self.text_encoder, &self.tokenizer] {
            if !model_dir.join(file).is_file() {
                return Err(failure(format!("missing {file}")));
            }
        }
        Ok(())
    }
}

struct Encoder {
    spec: ModelSpec,
    tokenizer: tokenizers::Tokenizer,
    text: Session,
    image: Session,
}

impl Encoder {
    fn load(model_dir: &Path) -> Result<Self, Failure> {
        let spec = ModelSpec::load(model_dir)?;
        spec.require_files(model_dir)?;
        let mut tokenizer = tokenizers::Tokenizer::from_file(model_dir.join(&spec.tokenizer))
            .map_err(|error| failure(format!("tokenizer could not be loaded: {error}")))?;
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: spec.context_length,
                ..Default::default()
            }))
            .map_err(|error| failure(format!("tokenizer truncation is invalid: {error}")))?;
        tokenizer.with_padding(None);
        let text = load_session(&model_dir.join(&spec.text_encoder))?;
        let image = load_session(&model_dir.join(&spec.image_encoder))?;
        Ok(Self {
            spec,
            tokenizer,
            text,
            image,
        })
    }

    fn token_ids(&self, text: &str) -> Result<Vec<i64>, Failure> {
        tokenize(&self.tokenizer, text, self.spec.context_length)
    }

    fn encode_text(&mut self, text: &str) -> Result<Vec<f32>, Failure> {
        let ids = self.token_ids(text)?;
        let input = Tensor::from_array(([1usize, self.spec.context_length], ids))
            .map_err(|error| failure(format!("text tensor could not be created: {error}")))?;
        let input_name = self.text.inputs()[0].name().to_string();
        let outputs = self
            .text
            .run(ort::inputs![input_name => input])
            .map_err(|error| failure(format!("text encoder failed: {error}")))?;
        pooled_embedding(&outputs, self.spec.embedding_dim)
    }

    fn encode_image(&mut self, bytes: &[u8]) -> Result<Vec<f32>, Failure> {
        let pixels = preprocess_image(bytes, self.spec.image_size)?;
        let size = self.spec.image_size as usize;
        let input = Tensor::from_array(([1usize, 3, size, size], pixels))
            .map_err(|error| failure(format!("image tensor could not be created: {error}")))?;
        let input_name = self.image.inputs()[0].name().to_string();
        let outputs = self
            .image
            .run(ort::inputs![input_name => input])
            .map_err(|error| failure(format!("image encoder failed: {error}")))?;
        pooled_embedding(&outputs, self.spec.embedding_dim)
    }
}

fn load_session(path: &Path) -> Result<Session, Failure> {
    Session::builder()
        .map_err(|error| {
            failure(format!(
                "ONNX Runtime session could not be created: {error}"
            ))
        })?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|error| failure(format!("ONNX Runtime session options failed: {error}")))?
        .commit_from_file(path)
        .map_err(|error| {
            failure(format!(
                "ONNX model {} could not be loaded: {error}",
                path.display()
            ))
        })
}

fn tokenize(
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
    context_length: usize,
) -> Result<Vec<i64>, Failure> {
    // SigLIP 2 was trained on lowercased text.
    let encoding = tokenizer
        .encode(text.to_lowercase(), true)
        .map_err(|error| failure(format!("text could not be tokenized: {error}")))?;
    let mut ids = encoding
        .get_ids()
        .iter()
        .map(|id| i64::from(*id))
        .collect::<Vec<_>>();
    ids.truncate(context_length);
    ids.resize(context_length, PAD_TOKEN_ID);
    Ok(ids)
}

fn pooled_embedding(
    outputs: &ort::session::SessionOutputs<'_>,
    dimensions: usize,
) -> Result<Vec<f32>, Failure> {
    let value = outputs
        .get("pooler_output")
        .ok_or_else(|| failure("model returned no pooler_output embedding"))?;
    let (shape, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|error| failure(format!("model returned an invalid embedding: {error}")))?;
    if shape.len() != 2 || shape[0] != 1 || shape[1] as usize != dimensions {
        return Err(failure(format!(
            "model returned embedding shape {shape:?}, expected [1, {dimensions}]"
        )));
    }
    l2_normalize(data.to_vec())
}

fn l2_normalize(mut vector: Vec<f32>) -> Result<Vec<f32>, Failure> {
    let norm = vector
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        return Err(failure("model returned a degenerate embedding"));
    }
    for value in &mut vector {
        *value = (f64::from(*value) / norm) as f32;
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return Err(failure("model returned a non-finite embedding"));
    }
    Ok(vector)
}

/// Returns planar CHW RGB float pixels normalized to [-1, 1].
fn preprocess_image(bytes: &[u8], size: u32) -> Result<Vec<f32>, Failure> {
    let decoded = image::load_from_memory(bytes)
        .map_err(|error| failure(format!("image bytes could not be decoded: {error}")))?
        .into_rgba8();
    // Composite over opaque black, matching the macOS helper's black-filled canvas.
    let mut rgb = image::RgbImage::new(decoded.width(), decoded.height());
    for (target, source) in rgb.pixels_mut().zip(decoded.pixels()) {
        let alpha = u16::from(source[3]);
        for channel in 0..3 {
            target[channel] = ((u16::from(source[channel]) * alpha + 127) / 255) as u8;
        }
    }
    let resized = image::imageops::resize(&rgb, size, size, FilterType::Triangle);
    let plane = (size * size) as usize;
    let mut pixels = vec![0.0_f32; plane * 3];
    for (index, pixel) in resized.pixels().enumerate() {
        for channel in 0..3 {
            pixels[channel * plane + index] = (f32::from(pixel[channel]) / 255.0 - 0.5) / 0.5;
        }
    }
    Ok(pixels)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    id: u64,
    #[serde(rename = "type")]
    request_type: String,
    text: Option<String>,
    image_base64: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
struct Response {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

trait EmbeddingBackend {
    fn text(&mut self, text: &str) -> Result<Vec<f32>, Failure>;
    fn image(&mut self, bytes: &[u8]) -> Result<Vec<f32>, Failure>;
}

impl EmbeddingBackend for Encoder {
    fn text(&mut self, text: &str) -> Result<Vec<f32>, Failure> {
        self.encode_text(text)
    }
    fn image(&mut self, bytes: &[u8]) -> Result<Vec<f32>, Failure> {
        self.encode_image(bytes)
    }
}

fn handle_line(backend: &mut dyn EmbeddingBackend, line: &str) -> Response {
    let request = match serde_json::from_str::<Request>(line) {
        Ok(request) => request,
        Err(error) => {
            return Response {
                id: 0,
                embedding: None,
                error: Some(format!("invalid request: {error}")),
            }
        }
    };
    let result = match request.request_type.as_str() {
        "text" => match request.text.as_deref() {
            Some(text) if !text.trim().is_empty() => backend.text(text),
            _ => Err(failure("text request is blank")),
        },
        "image" => match request.image_base64.as_deref().and_then(|encoded| {
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .ok()
        }) {
            Some(bytes) => backend.image(&bytes),
            None => Err(failure("image request has invalid base64")),
        },
        other => Err(failure(format!("unknown request type {other}"))),
    };
    match result {
        Ok(embedding) => Response {
            id: request.id,
            embedding: Some(embedding),
            error: None,
        },
        Err(error) => Response {
            id: request.id,
            embedding: None,
            error: Some(error.0),
        },
    }
}

fn serve(
    backend: &mut dyn EmbeddingBackend,
    input: impl BufRead,
    mut output: impl Write,
) -> Result<(), Failure> {
    for line in input.lines() {
        let line = line.map_err(|error| failure(format!("stdin read failed: {error}")))?;
        let response = handle_line(backend, &line);
        serde_json::to_writer(&mut output, &response)
            .map_err(|error| failure(format!("response encoding failed: {error}")))?;
        output
            .write_all(b"\n")
            .and_then(|()| output.flush())
            .map_err(|error| failure(format!("stdout write failed: {error}")))?;
    }
    Ok(())
}

fn prepare(model_dir: &Path) -> Result<Encoder, Failure> {
    // Loading both sessions validates the ONNX graphs and tokenizer before the bundle is
    // published; ONNX Runtime needs no separate compilation step.
    Encoder::load(model_dir)
}

fn parse_arguments(arguments: &[String]) -> Result<(String, PathBuf), Failure> {
    match arguments {
        [command, model_dir] if model_dir.starts_with("--model-dir=") => Ok((
            command.clone(),
            PathBuf::from(&model_dir["--model-dir=".len()..]),
        )),
        _ => Err(failure(
            "usage: video-creater-semantic-encoder <prepare|serve> --model-dir=<path>",
        )),
    }
}

fn run() -> Result<(), Failure> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (command, model_dir) = parse_arguments(&arguments)?;
    match command.as_str() {
        "prepare" => prepare(&model_dir).map(|_| ()),
        "serve" => {
            let mut encoder = prepare(&model_dir)?;
            let stdin = std::io::stdin();
            serve(&mut encoder, stdin.lock(), std::io::stdout().lock())
        }
        other => Err(failure(format!("unknown command {other}"))),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("semantic encoder failed: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend;

    impl EmbeddingBackend for FakeBackend {
        fn text(&mut self, text: &str) -> Result<Vec<f32>, Failure> {
            Ok(vec![text.len() as f32])
        }
        fn image(&mut self, bytes: &[u8]) -> Result<Vec<f32>, Failure> {
            Ok(vec![bytes.len() as f32])
        }
    }

    fn encode_png(width: u32, height: u32, pixel: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba(pixel));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("encode png");
        bytes.into_inner()
    }

    #[test]
    fn serve_speaks_the_ndjson_protocol() {
        let input = [
            r#"{"id":7,"type":"text","text":"a red square"}"#.to_string(),
            format!(
                r#"{{"id":8,"type":"image","imageBase64":"{}"}}"#,
                base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3])
            ),
            r#"{"id":9,"type":"text","text":"   "}"#.to_string(),
            r#"{"id":10,"type":"image","imageBase64":"***"}"#.to_string(),
            r#"{"id":11,"type":"audio"}"#.to_string(),
            "not json".to_string(),
        ]
        .join("\n");
        let mut output = Vec::new();
        serve(&mut FakeBackend, input.as_bytes(), &mut output).expect("serve");
        let lines = String::from_utf8(output).expect("utf8");
        let lines = lines.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], r#"{"id":7,"embedding":[12.0]}"#);
        assert_eq!(lines[1], r#"{"id":8,"embedding":[3.0]}"#);
        assert_eq!(lines[2], r#"{"id":9,"error":"text request is blank"}"#);
        assert_eq!(
            lines[3],
            r#"{"id":10,"error":"image request has invalid base64"}"#
        );
        assert_eq!(
            lines[4],
            r#"{"id":11,"error":"unknown request type audio"}"#
        );
        assert!(lines[5].starts_with(r#"{"id":0,"error":"invalid request"#));
    }

    #[test]
    fn arguments_follow_the_shared_usage() {
        let parsed = parse_arguments(&["serve".to_string(), "--model-dir=/models/x".to_string()])
            .expect("valid");
        assert_eq!(parsed, ("serve".to_string(), PathBuf::from("/models/x")));
        assert!(parse_arguments(&["serve".to_string()]).is_err());
        assert!(parse_arguments(&["serve".to_string(), "/models/x".to_string()]).is_err());
    }

    #[test]
    fn image_preprocessing_squashes_to_square_composites_alpha_and_normalizes() {
        let pixels = preprocess_image(&encode_png(40, 10, [255, 0, 0, 255]), 16).expect("red");
        assert_eq!(pixels.len(), 3 * 16 * 16);
        assert!(pixels[..256]
            .iter()
            .all(|value| (*value - 1.0).abs() < 1e-6));
        assert!(pixels[256..]
            .iter()
            .all(|value| (*value + 1.0).abs() < 1e-6));
        let transparent =
            preprocess_image(&encode_png(8, 8, [255, 255, 255, 0]), 4).expect("transparent");
        assert!(transparent.iter().all(|value| (*value + 1.0).abs() < 1e-6));
        assert!(preprocess_image(b"not an image", 16).is_err());
    }

    #[test]
    fn embeddings_are_l2_normalized() {
        let normalized = l2_normalize(vec![3.0, 4.0]).expect("normalize");
        assert_eq!(normalized, vec![0.6, 0.8]);
        assert!(l2_normalize(vec![0.0, 0.0]).is_err());
    }

    #[test]
    fn spec_defaults_and_rejects_unsafe_file_names() {
        let directory = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            directory.path().join("spec.json"),
            r#"{"model":"siglip2-base-patch16-256","version":"x","embeddingDim":768,"imageSize":256,"contextLength":64}"#,
        )
        .expect("spec");
        let spec = ModelSpec::load(directory.path()).expect("spec loads");
        assert_eq!(spec.image_encoder, DEFAULT_IMAGE_ENCODER);
        assert!(spec.require_files(directory.path()).is_err());
        std::fs::write(
            directory.path().join("spec.json"),
            r#"{"model":"m","embeddingDim":768,"imageSize":256,"contextLength":64,"textEncoder":"../x.onnx"}"#,
        )
        .expect("spec");
        assert!(ModelSpec::load(directory.path()).is_err());
    }

    /// Real-model check. Set `VIDEO_CREATER_SIGLIP2_ONNX_DIR` to a directory holding the
    /// pinned `vision_model_fp16.onnx`, `text_model_quantized.onnx`, and `tokenizer.json`
    /// (plus a spec.json) to run it.
    #[test]
    #[ignore = "requires the pinned SigLIP 2 ONNX bundle (~504 MB) via VIDEO_CREATER_SIGLIP2_ONNX_DIR"]
    fn real_siglip2_ranks_matching_text_image_pairs() {
        let model_dir = PathBuf::from(
            std::env::var_os("VIDEO_CREATER_SIGLIP2_ONNX_DIR").expect("model dir env"),
        );
        let mut encoder = Encoder::load(&model_dir).expect("load encoder");
        let red = encoder
            .encode_image(&encode_png(256, 256, [220, 20, 20, 255]))
            .expect("red image");
        let blue = encoder
            .encode_image(&encode_png(256, 256, [20, 40, 220, 255]))
            .expect("blue image");
        let texts = ["a red square", "a blue square", "a photo of a dog"]
            .map(|text| encoder.encode_text(text).expect("text"));
        let cosine =
            |left: &[f32], right: &[f32]| left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
        assert_eq!(red.len(), 768);
        assert!((cosine(&red, &red) - 1.0).abs() < 1e-4);
        let red_scores = texts
            .iter()
            .map(|text| cosine(&red, text))
            .collect::<Vec<_>>();
        let blue_scores = texts
            .iter()
            .map(|text| cosine(&blue, text))
            .collect::<Vec<_>>();
        eprintln!("red image vs [red, blue, dog]: {red_scores:?}");
        eprintln!("blue image vs [red, blue, dog]: {blue_scores:?}");
        assert!(red_scores[0] > red_scores[1] && red_scores[0] > red_scores[2]);
        assert!(blue_scores[1] > blue_scores[0] && blue_scores[1] > blue_scores[2]);
    }
}
