use anyhow::{bail, Context};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use video_creater_lib::search::semantic_runtime::{
    configured_semantic_encoder_status, install_palmier_siglip2_with, LocalSemanticEncoder,
    PalmierSiglip2Encoder, SemanticAcquisitionProgress,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddingEvidence {
    dimensions: usize,
    l2_norm: f32,
    finite: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceReport {
    schema_version: u32,
    status: String,
    installed_path: String,
    model_status: serde_json::Value,
    progress_phases: Vec<String>,
    text: EmbeddingEvidence,
    image: EmbeddingEvidence,
}

fn embedding_evidence(vector: Vec<f32>) -> EmbeddingEvidence {
    EmbeddingEvidence {
        dimensions: vector.len(),
        l2_norm: vector.iter().map(|value| value * value).sum::<f32>().sqrt(),
        finite: vector.iter().all(|value| value.is_finite()),
    }
}

fn main() -> anyhow::Result<()> {
    let mut arguments = std::env::args().skip(1);
    let source_dir = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: video-creater-semantic-model-e2e <artifact-dir> <image>")?;
    let image_path = arguments
        .next()
        .map(PathBuf::from)
        .context("usage: video-creater-semantic-model-e2e <artifact-dir> <image> [report.json]")?;
    let report_path = arguments.next().map(PathBuf::from);
    if arguments.next().is_some() {
        bail!("usage: video-creater-semantic-model-e2e <artifact-dir> <image> [report.json]");
    }

    let mut progress = Vec::<SemanticAcquisitionProgress>::new();
    let installed = install_palmier_siglip2_with(
        |url| {
            let file_name = url.rsplit('/').next().unwrap_or_default();
            fs::read(source_dir.join(file_name)).map_err(Into::into)
        },
        || false,
        |event| progress.push(event),
    )?;
    let encoder = PalmierSiglip2Encoder::installed()?;
    let text = embedding_evidence(encoder.encode_text("a red product on a table")?);
    let image = embedding_evidence(encoder.encode_image(&fs::read(image_path)?)?);
    let status = configured_semantic_encoder_status();
    if text.dimensions != 768
        || image.dimensions != 768
        || !text.finite
        || !image.finite
        || !(0.98..=1.02).contains(&text.l2_norm)
        || !(0.98..=1.02).contains(&image.l2_norm)
        || status.status != "installed"
        || !status.hash_verified
    {
        bail!("semantic model evidence did not satisfy the production contract");
    }
    let report = EvidenceReport {
        schema_version: 1,
        status: "passed".to_string(),
        installed_path: installed.display().to_string(),
        model_status: serde_json::to_value(status)?,
        progress_phases: progress.into_iter().map(|event| event.phase).collect(),
        text,
        image,
    };
    let report_json = serde_json::to_string_pretty(&report)?;
    if let Some(report_path) = report_path {
        if let Some(parent) = report_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(report_path, format!("{report_json}\n"))?;
    }
    println!("{report_json}");
    Ok(())
}
