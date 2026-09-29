#[cfg(all(target_os = "macos", feature = "coreml-inspect"))]
fn main() -> anyhow::Result<()> {
    use anyhow::{anyhow, bail};
    use coreml_native::{ComputeUnits, Model};
    use serde::Serialize;
    use std::path::PathBuf;
    use video_creater_lib::transcription::model::FLUID_AUDIO_COREML_RUNTIME_ID;

    const MODEL_ID: &str = "nvidia/parakeet-tdt-0.6b-v3";
    const USAGE: &str = "usage: video-creater-inspect-coreml-parakeet <model-root> [--json]";

    #[derive(Debug)]
    struct Args {
        model_root: PathBuf,
        json: bool,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct InspectionReport {
        schema_version: u32,
        model_id: String,
        runtime_id: String,
        bundles: Vec<BundleReport>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct BundleReport {
        name: String,
        path: String,
        inputs: Vec<FeatureReport>,
        outputs: Vec<FeatureReport>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct FeatureReport {
        name: String,
        feature_type: String,
        shape: Option<Vec<usize>>,
        data_type: Option<String>,
        optional: bool,
    }

    fn parse_args() -> anyhow::Result<Args> {
        let mut args = std::env::args().skip(1);
        let Some(model_root) = args.next() else {
            bail!(USAGE);
        };
        if model_root.starts_with('-') {
            return Err(anyhow!("missing model root\n{USAGE}"));
        }

        let mut json = false;
        for arg in args {
            match arg.as_str() {
                "--json" if !json => json = true,
                "--json" => bail!("duplicate --json flag\n{USAGE}"),
                _ if arg.starts_with('-') => bail!("unexpected flag '{arg}'\n{USAGE}"),
                _ => bail!("unexpected argument '{arg}'\n{USAGE}"),
            }
        }

        Ok(Args {
            model_root: PathBuf::from(model_root),
            json,
        })
    }

    fn feature_report(feature: &coreml_native::FeatureDescription) -> FeatureReport {
        FeatureReport {
            name: feature.name().to_string(),
            feature_type: feature.feature_type().to_string(),
            shape: feature.shape().map(<[usize]>::to_vec),
            data_type: feature.data_type().map(|data_type| data_type.to_string()),
            optional: feature.is_optional(),
        }
    }

    fn build_report(model_root: PathBuf) -> anyhow::Result<InspectionReport> {
        let mut bundles = Vec::new();

        for (name, bundle) in [
            ("Preprocessor", "Preprocessor.mlmodelc"),
            ("Encoder", "Encoder.mlmodelc"),
            ("Decoder", "Decoder.mlmodelc"),
            ("JointDecisionv3", "JointDecisionv3.mlmodelc"),
        ] {
            let path = model_root.join(bundle);
            let model = Model::load(&path, ComputeUnits::All)?;
            bundles.push(BundleReport {
                name: name.to_string(),
                path: path.display().to_string(),
                inputs: model.inputs().iter().map(feature_report).collect(),
                outputs: model.outputs().iter().map(feature_report).collect(),
            });
        }

        Ok(InspectionReport {
            schema_version: 1,
            model_id: MODEL_ID.to_string(),
            runtime_id: FLUID_AUDIO_COREML_RUNTIME_ID.to_string(),
            bundles,
        })
    }

    fn print_text_report(report: &InspectionReport) {
        for bundle in &report.bundles {
            println!("MODEL {}", bundle.path);
            println!("INPUTS");
            for input in &bundle.inputs {
                println!(
                    "  {} {} shape={:?} optional={}",
                    input.name, input.feature_type, input.shape, input.optional
                );
                if let Some(data_type) = &input.data_type {
                    println!("    data_type={data_type}");
                }
            }
            println!("OUTPUTS");
            for output in &bundle.outputs {
                println!(
                    "  {} {} shape={:?}",
                    output.name, output.feature_type, output.shape
                );
                if let Some(data_type) = &output.data_type {
                    println!("    data_type={data_type}");
                }
            }
        }
    }

    let args = parse_args()?;
    let report = build_report(args.model_root)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_text_report(&report);
    }

    Ok(())
}

#[cfg(all(target_os = "macos", not(feature = "coreml-inspect")))]
fn main() -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "Core ML Parakeet inspection requires the coreml-inspect feature"
    ))
}

#[cfg(not(target_os = "macos"))]
fn main() -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "Core ML Parakeet inspection requires macOS"
    ))
}
