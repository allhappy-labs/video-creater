use serde_json::json;
use std::{fs, path::PathBuf};
use video_creater_lib::{
    speech_analysis::{analyze_wav_cached_production, default_speech_helper_path},
    speech_models::ProductionSpeechModelStore,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
        .to_path_buf();
    let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
    let model_root = PathBuf::from(home).join("Library/Application Support/video-creater/models");
    let store = ProductionSpeechModelStore::new(model_root);
    let status = store.download_and_verify()?;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let fixture = args
        .windows(2)
        .find(|pair| pair[0] == "--fixture")
        .map(|pair| PathBuf::from(&pair[1]))
        .unwrap_or_else(|| repo.join("src-tauri/tests/fixtures/media/edison-speech-1920s-30s.mp4"));
    let expected_speakers = args
        .windows(2)
        .find(|pair| pair[0] == "--expected-speakers")
        .and_then(|pair| pair[1].parse::<usize>().ok())
        .unwrap_or(1);
    let fixture_id = args
        .windows(2)
        .find(|pair| pair[0] == "--fixture-id")
        .map(|pair| pair[1].clone())
        .unwrap_or_else(|| "edison-speech-1920s-30s".to_string());
    let fixture_license = args
        .windows(2)
        .find(|pair| pair[0] == "--fixture-license")
        .map(|pair| pair[1].clone())
        .unwrap_or_else(|| "public-domain".to_string());
    let evidence = repo.join("output/speech-model-evidence");
    let project = evidence.join("project");
    fs::create_dir_all(&project)?;
    let helper = default_speech_helper_path();
    let sidecar =
        analyze_wav_cached_production(&project, &fixture_id, &fixture, &helper, store.root())?;
    let speakers = sidecar
        .speech_ranges
        .iter()
        .filter_map(|range| range.speaker_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let passed = speakers.len() >= expected_speakers;
    let report = json!({
        "schemaVersion": 1,
        "status": if passed { "passed" } else { "failed" },
        "runtime": "fluid_audio_speech_analysis",
        "models": status,
        "fixture": {
            "id": fixture_id.clone(),
            "path": fixture,
            "license": fixture_license
        },
        "analysis": {
            "analyzerVersion": sidecar.analyzer_version,
            "embeddingVersion": sidecar.embedding_version,
            "durationSeconds": sidecar.duration_seconds,
            "frameCount": sidecar.frames.len(),
            "speechRanges": sidecar.speech_ranges,
            "deadAirRanges": sidecar.dead_air_ranges,
            "detectedSpeakers": speakers.len(),
            "expectedSpeakers": expected_speakers,
            "embeddingDimensions": sidecar.frames.iter().find(|frame| !frame.embedding.is_empty()).map(|frame| frame.embedding.len())
        },
        "checks": {
            "modelHashesVerified": true,
            "realVad": !sidecar.frames.is_empty(),
            "realSpeechDetected": !sidecar.speech_ranges.is_empty(),
            "productionDiarizationEmbedding": sidecar.frames.iter().any(|frame| frame.embedding.len() == 256),
            "expectedSpeakerCountPassed": speakers.len() >= expected_speakers,
            "offlineReuseReady": store.status().ready,
            "helperPath": helper
        }
    });
    let report_bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(evidence.join("report.json"), &report_bytes)?;
    fs::write(
        evidence.join(format!("report-{fixture_id}.json")),
        &report_bytes,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !passed {
        return Err(format!(
            "detected {} speakers; expected at least {expected_speakers}",
            speakers.len()
        )
        .into());
    }
    Ok(())
}
