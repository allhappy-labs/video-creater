use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
};
use video_creater_lib::generation::replicate::{
    build_replicate_generated_output_import, build_replicate_generation_completion_actions,
    build_replicate_generation_request, build_replicate_prediction_submission,
    download_replicate_generated_output_with_client_cancellable, fetch_replicate_prediction,
    run_replicate_generation_submission_with_client,
    run_replicate_generation_submission_with_client_cancellable,
    submit_replicate_prediction_submission, upload_replicate_local_file_with_client,
    ReplicateGenerationRunOptions, ReplicateGenerationWorkerError, ReplicatePredictionStatusKind,
    REPLICATE_FLUX_11_PRO_MODEL_ID, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
    REPLICATE_FLUX_DEV_MODEL_ID, REPLICATE_FLUX_SCHNELL_MODEL_ID, REPLICATE_PROVIDER,
    REPLICATE_SEEDANCE_20_FAST_MODEL_ID, REPLICATE_SEEDANCE_20_MODEL_ID,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_asset(model_id: &str, settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-shot-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: None,
        target_folder_id: None,
        placement_intent: None,
        prompt: "  floating product shot with crisp rim light  ".to_string(),
        model: GenerationModel {
            provider: "replicate".to_string(),
            id: model_id.to_string(),
        },
        references: GeneratedAssetReferences {
            media_ids: Vec::new(),
            first_frame_media_id: None,
            last_frame_media_id: None,
            provider_input_urls: Vec::new(),
            ..Default::default()
        },
        settings,
        outputs: Vec::new(),
        created_at: "2026-06-23T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

#[test]
fn replicate_flux_schnell_asset_builds_prediction_request() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_replicate_generation_request(&asset).expect("build replicate request");

    assert_eq!(request.version, REPLICATE_FLUX_SCHNELL_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "width": 1024,
            "height": 768,
            "num_outputs": 1,
            "output_format": "png"
        })
    );
}

#[test]
fn replicate_flux_schnell_asset_builds_multi_output_prediction_request() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            resolution: None,
            generate_audio: None,
            num_images: Some(4),
            ..GeneratedAssetSettings::default()
        },
    );

    let request =
        build_replicate_generation_request(&asset).expect("build multi-output replicate request");

    assert_eq!(request.version, REPLICATE_FLUX_SCHNELL_MODEL_ID);
    assert_eq!(request.input["num_outputs"], json!(4));
}

#[test]
fn replicate_flux_dev_asset_builds_prediction_request_and_output_import() {
    let asset = generated_asset(
        REPLICATE_FLUX_DEV_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(768),
            height: Some(1344),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("9:16".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_replicate_generation_request(&asset).expect("build flux dev request");

    assert_eq!(request.version, REPLICATE_FLUX_DEV_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "width": 768,
            "height": 1344,
            "num_outputs": 1,
            "output_format": "png"
        })
    );

    let import = build_replicate_generated_output_import(
        &asset,
        &json!({ "output": ["https://cdn.example.com/flux-dev.png"] }),
        "generated/generated-shot-1/replicate-output.png",
    )
    .expect("build flux dev output import");
    assert_eq!(import.output.media_id, "generated-shot-1-replicate-output");
    assert_eq!(
        import.output.source_url.as_deref(),
        Some("https://cdn.example.com/flux-dev.png")
    );
    assert_eq!(
        import.output.relative_path,
        "generated/generated-shot-1/replicate-output.png"
    );
}

#[test]
fn replicate_flux_11_pro_asset_builds_prediction_request() {
    let asset = generated_asset(
        REPLICATE_FLUX_11_PRO_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request = build_replicate_generation_request(&asset).expect("build flux 1.1 pro request");

    assert_eq!(request.version, REPLICATE_FLUX_11_PRO_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "width": 1024,
            "height": 1024,
            "num_outputs": 1,
            "output_format": "png"
        })
    );
}

#[test]
fn replicate_flux_11_pro_ultra_asset_builds_prediction_request() {
    let asset = generated_asset(
        REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1536),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("3:2".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let request =
        build_replicate_generation_request(&asset).expect("build flux 1.1 pro ultra request");

    assert_eq!(request.version, REPLICATE_FLUX_11_PRO_ULTRA_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "width": 1536,
            "height": 1024,
            "num_outputs": 1,
            "output_format": "png"
        })
    );
}

#[test]
fn replicate_seedance_fast_video_asset_uses_fast_model_identifier() {
    let mut asset = generated_asset(
        REPLICATE_SEEDANCE_20_FAST_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(5.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references.first_frame_media_id = Some("first-frame".to_string());
    asset.references.provider_input_urls =
        vec!["https://replicate.delivery/pbxt/first.png".to_string()];

    let request = build_replicate_generation_request(&asset).expect("build seedance fast request");

    assert_eq!(request.version, REPLICATE_SEEDANCE_20_FAST_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "duration": 5,
            "resolution": "720p",
            "aspect_ratio": "16:9",
            "generate_audio": true,
            "first_frame_url": "https://replicate.delivery/pbxt/first.png"
        })
    );

    let import = build_replicate_generated_output_import(
        &asset,
        &json!({ "output": "https://replicate.delivery/pbxt/seedance-fast.mp4" }),
        "generated/generated-shot-1/replicate-output.mp4",
    )
    .expect("build seedance fast output import");
    assert_eq!(
        import.output.source_url.as_deref(),
        Some("https://replicate.delivery/pbxt/seedance-fast.mp4")
    );
    assert_eq!(import.output.duration_seconds, 5.0);
    assert_eq!(import.output.fps, 24.0);
}

#[test]
fn replicate_seedance_video_asset_builds_reference_prediction_request_and_output_import() {
    let mut asset = generated_asset(
        REPLICATE_SEEDANCE_20_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1280),
            height: Some(720),
            duration_seconds: Some(4.0),
            fps: Some(24.0),
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("720p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.references.first_frame_media_id = Some("first-frame".to_string());
    asset.references.last_frame_media_id = Some("last-frame".to_string());
    asset.references.reference_image_media_refs = vec!["style-image".to_string()];
    asset.references.reference_video_media_refs = vec!["motion-video".to_string()];
    asset.references.reference_audio_media_refs = vec!["audio-ref".to_string()];
    asset.references.provider_input_urls = vec![
        "https://replicate.delivery/pbxt/first.png".to_string(),
        "https://replicate.delivery/pbxt/last.png".to_string(),
        "https://replicate.delivery/pbxt/style.png".to_string(),
        "https://replicate.delivery/pbxt/motion.mp4".to_string(),
        "https://replicate.delivery/pbxt/audio.wav".to_string(),
    ];

    let request = build_replicate_generation_request(&asset).expect("build seedance video request");

    assert_eq!(request.version, REPLICATE_SEEDANCE_20_MODEL_ID);
    assert_eq!(
        request.input,
        json!({
            "prompt": "floating product shot with crisp rim light",
            "duration": 4,
            "resolution": "720p",
            "aspect_ratio": "16:9",
            "generate_audio": true,
            "first_frame_url": "https://replicate.delivery/pbxt/first.png",
            "last_frame_url": "https://replicate.delivery/pbxt/last.png",
            "reference_image_urls": ["https://replicate.delivery/pbxt/style.png"],
            "reference_video_urls": ["https://replicate.delivery/pbxt/motion.mp4"],
            "reference_audio_urls": ["https://replicate.delivery/pbxt/audio.wav"]
        })
    );

    let import = build_replicate_generated_output_import(
        &asset,
        &json!({ "output": "https://replicate.delivery/pbxt/seedance.mp4" }),
        "generated/generated-shot-1/replicate-output.mp4",
    )
    .expect("build seedance output import");
    assert_eq!(
        import.output.source_url.as_deref(),
        Some("https://replicate.delivery/pbxt/seedance.mp4")
    );
    assert_eq!(
        import.output.relative_path,
        "generated/generated-shot-1/replicate-output.mp4"
    );
    assert_eq!(import.output.width, 1280);
    assert_eq!(import.output.height, 720);
    assert_eq!(import.output.duration_seconds, 4.0);
    assert_eq!(import.output.fps, 24.0);
}

#[test]
fn replicate_prediction_submission_uses_api_endpoint_without_credentials() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("1:1".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let submission =
        build_replicate_prediction_submission(&asset).expect("build replicate submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.replicate.com/v1/predictions");
    assert_eq!(submission.version, REPLICATE_FLUX_SCHNELL_MODEL_ID);
    assert_eq!(submission.provider, REPLICATE_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert!(!serde_json::to_string(&submission)
        .unwrap()
        .contains("unit-test-token"));
}

#[test]
fn submits_replicate_prediction_with_secret_only_in_authorization_header() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings::default(),
    );
    let mut submission =
        build_replicate_prediction_submission(&asset).expect("build replicate submission");
    let Some((base_url, request_handle)) = spawn_replicate_json_server(
        r#"{"id":"pred-123","status":"starting","output":null,"error":null,"urls":{"get":"https://api.replicate.com/v1/predictions/pred-123","cancel":"https://api.replicate.com/v1/predictions/pred-123/cancel"}}"#,
    ) else {
        return;
    };
    submission.url = format!("{base_url}/v1/predictions");

    let response = submit_replicate_prediction_submission(&submission, "unit-test-token")
        .expect("submit replicate prediction");
    let request = request_handle.join().expect("join request server");

    assert_eq!(response.id, "pred-123");
    assert!(request.starts_with("POST /v1/predictions HTTP/1.1"));
    assert!(request.contains("authorization: Bearer unit-test-token"));
    assert!(request.contains("\"version\":\"black-forest-labs/flux-schnell\""));
    assert!(!request.contains("\"unit-test-token\""));
}

#[test]
fn uploads_local_file_to_replicate_files_with_rest_multipart() {
    let source_dir = tempfile::tempdir().expect("temp source dir");
    let source_path = source_dir.path().join("source.png");
    std::fs::write(&source_path, b"source-pixels").expect("source image");
    let Some((base_url, request_handle)) = spawn_replicate_file_upload_server() else {
        return;
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let uploaded_url = upload_replicate_local_file_with_client(
        &client,
        &format!("{base_url}/v1"),
        &source_path,
        "unit-test-token",
    )
    .expect("upload replicate file");
    let request = request_handle.join().expect("join upload server");

    assert_eq!(uploaded_url, "https://api.replicate.com/v1/files/file-123");
    assert!(request.starts_with("POST /v1/files HTTP/1.1"));
    assert!(request.contains("authorization: Bearer unit-test-token"));
    assert!(request.contains("multipart/form-data"));
    assert!(request.contains(r#"name="content"; filename="source.png""#));
    assert!(request.contains("Content-Type: image/png"));
    assert!(request.contains("source-pixels"));
    assert!(request.contains(r#"name="metadata""#));
    assert!(request.contains("{}"));
    assert!(!request_body(&request).contains("unit-test-token"));
    assert!(!request.contains("replicate files upload"));
}

#[test]
fn fetches_replicate_prediction_with_secret_only_in_authorization_header() {
    let Some((base_url, request_handle)) = spawn_replicate_json_server(
        r#"{"id":"pred-123","status":"succeeded","output":["https://replicate.delivery/pbxt/output.png"],"error":null,"urls":{"get":"https://api.replicate.com/v1/predictions/pred-123","cancel":"https://api.replicate.com/v1/predictions/pred-123/cancel"}}"#,
    ) else {
        return;
    };
    let prediction_url = format!("{base_url}/v1/predictions/pred-123");

    let prediction =
        fetch_replicate_prediction(&prediction_url, "unit-test-token").expect("fetch prediction");
    let request = request_handle.join().expect("join request server");

    assert_eq!(prediction.status, ReplicatePredictionStatusKind::Succeeded);
    assert!(request.starts_with("GET /v1/predictions/pred-123 HTTP/1.1"));
    assert!(request.contains("authorization: Bearer unit-test-token"));
    assert!(!request.contains("\"unit-test-token\""));
}

#[test]
fn replicate_generation_result_builds_generated_output_import() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );

    let output_import = build_replicate_generated_output_import(
        &asset,
        &json!(["https://replicate.delivery/pbxt/output.png"]),
        "generated/generated-shot-1/replicate-output.png",
    )
    .expect("build output import");

    assert_eq!(
        output_import.source_url,
        "https://replicate.delivery/pbxt/output.png"
    );
    assert_eq!(
        output_import.output.media_id,
        "generated-shot-1-replicate-output"
    );
    assert_eq!(output_import.output.width, 1024);
    assert_eq!(output_import.output.height, 768);
}

#[test]
fn replicate_generation_completion_actions_preserve_multi_image_outputs() {
    let mut asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            resolution: None,
            generate_audio: None,
            num_images: Some(2),
            ..GeneratedAssetSettings::default()
        },
    );
    asset.status = GeneratedAssetStatus::Running;

    let actions = build_replicate_generation_completion_actions(
        &asset,
        &json!([
            "https://replicate.delivery/pbxt/output-one.png",
            "https://replicate.delivery/pbxt/output-two.png"
        ]),
        "generated/generated-shot-1/replicate-output.png",
        "2026-06-25T12:05:00Z",
        Some("run-1"),
        Some("timeline-item-that-needs-explicit-output-selection"),
    )
    .expect("build replicate multi-image completion actions");

    match &actions[0] {
        video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
            outputs,
            completion: Some(completion),
            replacement,
            ..
        } => {
            assert_eq!(outputs.len(), 2);
            assert_eq!(outputs[0].media_id, "generated-shot-1-replicate-output");
            assert_eq!(
                outputs[0].relative_path,
                "generated/generated-shot-1/replicate-output.png"
            );
            assert_eq!(
                outputs[0].source_url.as_deref(),
                Some("https://replicate.delivery/pbxt/output-one.png")
            );
            assert_eq!(outputs[1].media_id, "generated-shot-1-replicate-output-2");
            assert_eq!(
                outputs[1].relative_path,
                "generated/generated-shot-1/replicate-output-2.png"
            );
            assert_eq!(
                outputs[1].source_url.as_deref(),
                Some("https://replicate.delivery/pbxt/output-two.png")
            );
            assert_eq!(
                completion.generated_output_media_id,
                "generated-shot-1-replicate-output"
            );
            assert_eq!(replacement, &None);
        }
        action => panic!("expected multi-output replicate completion action: {action:?}"),
    }
}

#[test]
fn run_replicate_prediction_downloads_output_and_builds_completion_actions() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            duration_seconds: None,
            fps: None,
            aspect_ratio: Some("4:3".to_string()),
            resolution: None,
            generate_audio: None,
            ..GeneratedAssetSettings::default()
        },
    );
    let mut submission =
        build_replicate_prediction_submission(&asset).expect("build replicate submission");
    let Some((base_url, request_handle)) = spawn_replicate_run_server() else {
        return;
    };
    submission.url = format!("{base_url}/v1/predictions");
    let project_dir = tempfile::tempdir().expect("project tempdir");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_replicate_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-06-25T12:05:00Z",
            Some("run-1"),
            None,
        ),
        &submission,
        "unit-test-token",
        ReplicateGenerationRunOptions {
            max_status_polls: 2,
            poll_interval: Duration::from_millis(0),
        },
    )
    .expect("run replicate generation");
    let requests = request_handle.join().expect("join run server");

    assert_eq!(run.prediction_id, "pred-123");
    assert_eq!(run.status.status, ReplicatePredictionStatusKind::Succeeded);
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("downloaded replicate output"),
        b"fake-png"
    );
    assert!(run
        .completion
        .output_path
        .ends_with("generated/generated-shot-1/replicate-output.png"));
    let actions_json = serde_json::to_string(&run.completion.actions).expect("actions json");
    assert!(actions_json.contains("completeGeneratedAsset"));
    assert!(actions_json.contains("generated/generated-shot-1/replicate-output.png"));
    assert!(!actions_json.contains("unit-test-token"));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("POST /v1/predictions ")));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("GET /v1/predictions/pred-123 ")));
    assert!(requests
        .iter()
        .any(|request| request.starts_with("GET /media/output.png ")));
}

#[test]
fn replicate_generation_cancellation_interrupts_poll_wait() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            ..GeneratedAssetSettings::default()
        },
    );
    let mut submission = build_replicate_prediction_submission(&asset).expect("build submission");
    let Some((base_url, request_handle)) = spawn_replicate_poll_server() else {
        return;
    };
    submission.url = format!("{base_url}/v1/predictions");
    let project_dir = tempfile::tempdir().expect("project tempdir");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let guard = register_generation_cancellation("project-replicate-cancel", &asset.id)
        .expect("register cancellation");
    let token = guard.token();
    let job_id = asset.id.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        request_generation_cancellation("project-replicate-cancel", &job_id)
    });
    let started = std::time::Instant::now();

    let error = run_replicate_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-12T00:00:00Z",
            None,
            None,
        ),
        &submission,
        "unit-test-token",
        ReplicateGenerationRunOptions {
            max_status_polls: 3,
            poll_interval: Duration::from_secs(5),
        },
        Some(&token),
    )
    .expect_err("cancel polling");

    assert_eq!(error, ReplicateGenerationWorkerError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    cancel_thread.join().expect("cancel thread");
    assert_eq!(request_handle.join().expect("request capture").len(), 2);
}

#[test]
fn replicate_generation_cancellation_during_download_removes_partial_file() {
    let asset = generated_asset(
        REPLICATE_FLUX_SCHNELL_MODEL_ID,
        GeneratedAssetSettings {
            width: Some(1024),
            height: Some(768),
            ..GeneratedAssetSettings::default()
        },
    );
    let Some((base_url, server)) = spawn_replicate_streaming_download_server() else {
        return;
    };
    let output_import = build_replicate_generated_output_import(
        &asset,
        &json!([format!("{base_url}/output.png")]),
        "generated/generated-shot-1/replicate-output.png",
    )
    .expect("output import");
    let project_dir = tempfile::tempdir().expect("project dir");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let guard = register_generation_cancellation("project-replicate-download", &asset.id)
        .expect("register cancellation");
    let token = guard.token();
    let job_id = asset.id.clone();
    let cancel_thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        request_generation_cancellation("project-replicate-download", &job_id)
    });

    let error = download_replicate_generated_output_with_client_cancellable(
        &client,
        project_dir.path(),
        &output_import,
        &token,
    )
    .expect_err("cancel download");
    let output_path = project_dir
        .path()
        .join("generated/generated-shot-1/replicate-output.png");
    let part_path = output_path.with_extension("png.part");

    assert_eq!(error, ReplicateGenerationWorkerError::Cancelled);
    assert!(!output_path.exists());
    assert!(!part_path.exists());
    cancel_thread.join().expect("cancel thread");
    server.join().expect("streaming server");
}

fn spawn_replicate_json_server(body: &'static str) -> Option<(String, thread::JoinHandle<String>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) => panic!("bind replicate response server: {error}"),
    };
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    Some((
        base_url,
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept replicate request");
            let request = read_request(&mut stream);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write replicate response");
            request
        }),
    ))
}

fn spawn_replicate_file_upload_server() -> Option<(String, thread::JoinHandle<String>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) => panic!("bind replicate file upload server: {error}"),
    };
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    Some((
        base_url,
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept replicate upload request");
            let request = read_request(&mut stream);
            let body = r#"{"id":"file-123","urls":{"get":"https://api.replicate.com/v1/files/file-123"},"content_type":"image/png","size":13,"checksums":{"sha256":"abc"},"metadata":{},"created_at":"2026-07-06T12:00:00Z","expires_at":"2026-07-07T12:00:00Z"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write replicate upload response");
            request
        }),
    ))
}

fn spawn_replicate_run_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) => panic!("bind replicate run server: {error}"),
    };
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let responses = vec![
        (
            "application/json",
            format!(
                r#"{{"id":"pred-123","status":"starting","output":null,"error":null,"urls":{{"get":"{base_url}/v1/predictions/pred-123","cancel":"{base_url}/v1/predictions/pred-123/cancel"}}}}"#
            )
            .into_bytes(),
        ),
        (
            "application/json",
            format!(
                r#"{{"id":"pred-123","status":"succeeded","output":["{base_url}/media/output.png"],"error":null,"urls":{{"get":"{base_url}/v1/predictions/pred-123","cancel":"{base_url}/v1/predictions/pred-123/cancel"}}}}"#
            )
            .into_bytes(),
        ),
        ("image/png", b"fake-png".to_vec()),
    ];

    Some((
        base_url,
        thread::spawn(move || {
            let mut requests = Vec::new();
            for (content_type, body) in responses {
                let (mut stream, _) = listener.accept().expect("accept replicate request");
                requests.push(read_request(&mut stream));
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                stream
                    .write_all(headers.as_bytes())
                    .expect("write replicate headers");
                stream.write_all(&body).expect("write replicate body");
            }
            requests
        }),
    ))
}

fn spawn_replicate_poll_server() -> Option<(String, thread::JoinHandle<Vec<String>>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind replicate poll server: {error}"),
    };
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let responses = vec![
        format!(
            r#"{{"id":"pred-123","status":"starting","output":null,"error":null,"urls":{{"get":"{base_url}/v1/predictions/pred-123","cancel":"{base_url}/v1/predictions/pred-123/cancel"}}}}"#,
        ),
        format!(
            r#"{{"id":"pred-123","status":"processing","output":null,"error":null,"urls":{{"get":"{base_url}/v1/predictions/pred-123","cancel":"{base_url}/v1/predictions/pred-123/cancel"}}}}"#,
        ),
    ];
    Some((
        base_url,
        thread::spawn(move || {
            let mut requests = Vec::new();
            for body in responses {
                let (mut stream, _) = listener.accept().expect("accept replicate request");
                requests.push(read_request(&mut stream));
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                    body.len(),
                );
                stream.write_all(headers.as_bytes()).expect("write headers");
                stream.write_all(body.as_bytes()).expect("write body");
            }
            requests
        }),
    ))
}

fn spawn_replicate_streaming_download_server() -> Option<(String, thread::JoinHandle<()>)> {
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
        Err(error) => panic!("bind replicate streaming server: {error}"),
    };
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    Some((
        base_url,
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept replicate download");
            let _ = read_request(&mut stream);
            let chunk = vec![b'x'; 64 * 1024];
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
                chunk.len() * 2,
            );
            stream.write_all(headers.as_bytes()).expect("write headers");
            stream.write_all(&chunk).expect("write first chunk");
            stream.flush().expect("flush first chunk");
            thread::sleep(Duration::from_millis(150));
            let _ = stream.write_all(&chunk);
        }),
    ))
}

fn read_request(stream: &mut impl Read) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream.read(&mut buffer).expect("read request");
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if request_body_complete(&bytes) {
            break;
        }
    }

    String::from_utf8(bytes).expect("utf8 request")
}

fn request_body_complete(bytes: &[u8]) -> bool {
    let request = String::from_utf8_lossy(bytes);
    let Some(header_end) = request.find("\r\n\r\n") else {
        return false;
    };
    let headers = &request[..header_end];
    let content_length = headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("content-length") {
            value.trim().parse::<usize>().ok()
        } else {
            None
        }
    });
    match content_length {
        Some(length) => bytes.len() >= header_end + 4 + length,
        None => true,
    }
}

fn request_body(request: &str) -> &str {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("")
}
