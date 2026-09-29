use reqwest::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
};
use video_creater_lib::generation::google::{
    build_google_gemini_tts_generation_submission, build_google_lyria_generation_submission,
    build_google_veo_generation_submission,
    run_google_gemini_tts_generation_submission_with_client,
    run_google_gemini_tts_generation_submission_with_client_cancellable,
    run_google_lyria_generation_submission_with_client,
    run_google_lyria_generation_submission_with_client_cancellable,
    run_google_veo_generation_submission_with_client_cancellable,
    run_google_veo_generation_submission_with_client_cancellable_and_download_policy,
    GoogleGenerationWorkerError, GOOGLE_GEMINI_TTS_MODEL_ID, GOOGLE_LYRIA_3_PRO_API_MODEL_ID,
    GOOGLE_LYRIA_3_PRO_MODEL_ID, GOOGLE_PROVIDER, GOOGLE_VEO_31_FAST_API_MODEL_ID,
    GOOGLE_VEO_31_FAST_MODEL_ID,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_google_veo_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-google-veo-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Veo launch shot".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  A cinematic product launch video with a slow dolly move and native sound.  "
            .to_string(),
        model: GenerationModel {
            provider: GOOGLE_PROVIDER.to_string(),
            id: GOOGLE_VEO_31_FAST_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_google_gemini_tts_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-google-tts-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Gemini voiceover".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  Read this concise product launch narration.  ".to_string(),
        model: GenerationModel {
            provider: GOOGLE_PROVIDER.to_string(),
            id: GOOGLE_GEMINI_TTS_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_google_lyria_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-google-lyria-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Lyria music bed".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  Create a two minute cinematic synthwave song with a strong chorus.  "
            .to_string(),
        model: GenerationModel {
            provider: GOOGLE_PROVIDER.to_string(),
            id: GOOGLE_LYRIA_3_PRO_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_google_veo_asset_with_references(
    references: GeneratedAssetReferences,
    settings: GeneratedAssetSettings,
) -> GeneratedAsset {
    GeneratedAsset {
        references,
        ..generated_google_veo_asset(settings)
    }
}

#[test]
fn google_veo_fast_asset_builds_predict_long_running_request() {
    let asset = generated_google_veo_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        duration_seconds: Some(8.0),
        fps: Some(24.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: Some(true),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_google_veo_generation_submission(&asset).expect("build Google Veo submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(
        submission.url,
        "https://generativelanguage.googleapis.com/v1beta/models/veo-3.1-fast-generate-preview:predictLongRunning"
    );
    assert_eq!(submission.model, GOOGLE_VEO_31_FAST_MODEL_ID);
    assert_eq!(submission.api_model, GOOGLE_VEO_31_FAST_API_MODEL_ID);
    assert_eq!(submission.provider, GOOGLE_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "instances": [{
                "prompt": "A cinematic product launch video with a slow dolly move and native sound."
            }],
            "parameters": {
                "durationSeconds": 8,
                "aspectRatio": "16:9",
                "resolution": "720p",
                "generateAudio": true
            }
        })
    );
}

#[test]
fn google_veo_fast_asset_builds_image_input_request() {
    let asset = generated_google_veo_asset_with_references(
        GeneratedAssetReferences {
            media_ids: vec![
                "first-frame".to_string(),
                "last-frame".to_string(),
                "style-ref".to_string(),
                "product-ref".to_string(),
            ],
            first_frame_media_id: Some("first-frame".to_string()),
            last_frame_media_id: Some("last-frame".to_string()),
            reference_image_media_refs: vec!["style-ref".to_string(), "product-ref".to_string()],
            provider_input_urls: vec![
                "data:image/png;base64,ZmFrZS1maXJzdA==".to_string(),
                "data:image/png;base64,ZmFrZS1sYXN0".to_string(),
                "data:image/png;base64,ZmFrZS1zdHlsZQ==".to_string(),
                "data:image/jpeg;base64,ZmFrZS1wcm9kdWN0".to_string(),
            ],
            ..GeneratedAssetReferences::default()
        },
        GeneratedAssetSettings {
            duration_seconds: Some(8.0),
            aspect_ratio: Some("9:16".to_string()),
            resolution: Some("1080p".to_string()),
            generate_audio: Some(true),
            ..GeneratedAssetSettings::default()
        },
    );

    let submission =
        build_google_veo_generation_submission(&asset).expect("build Google Veo submission");

    assert_eq!(
        submission.input,
        json!({
            "instances": [{
                "prompt": "A cinematic product launch video with a slow dolly move and native sound.",
                "image": {
                    "inlineData": {
                        "mimeType": "image/png",
                        "data": "ZmFrZS1maXJzdA=="
                    }
                },
                "lastFrame": {
                    "image": {
                        "inlineData": {
                            "mimeType": "image/png",
                            "data": "ZmFrZS1sYXN0"
                        }
                    }
                },
                "referenceImages": [
                    {
                        "image": {
                            "inlineData": {
                                "mimeType": "image/png",
                                "data": "ZmFrZS1zdHlsZQ=="
                            }
                        },
                        "referenceType": "asset"
                    },
                    {
                        "image": {
                            "inlineData": {
                                "mimeType": "image/jpeg",
                                "data": "ZmFrZS1wcm9kdWN0"
                            }
                        },
                        "referenceType": "asset"
                    }
                ]
            }],
            "parameters": {
                "durationSeconds": 8,
                "aspectRatio": "9:16",
                "resolution": "1080p",
                "generateAudio": true
            }
        })
    );
}

#[test]
fn google_veo_run_polls_downloads_mp4_and_records_source_url() {
    let asset = generated_google_veo_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        duration_seconds: Some(8.0),
        fps: Some(24.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: Some(true),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_google_veo_generation_submission(&asset).expect("build Google Veo submission");
    let (base_url, request_handle) = spawn_google_veo_server();
    submission.url =
        format!("{base_url}/v1beta/models/veo-3.1-fast-generate-preview:predictLongRunning");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let download_policy =
        video_creater_lib::generation::download::DownloadNetworkPolicy::public_only()
            .with_trusted_origin(&base_url)
            .expect("fixture download origin");
    let run = run_google_veo_generation_submission_with_client_cancellable_and_download_policy(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("google-run-1"),
            None,
        ),
        &submission,
        "test-gemini-token",
        None,
        &download_policy,
    )
    .expect("run Google Veo generation");

    assert_eq!(run.request_id, "operations/google-veo-operation-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("Google Veo output"),
        b"google-veo-mp4"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-google-veo-1/google-veo-output.mp4")
    );
    let completion = run
        .completion
        .actions
        .iter()
        .find_map(|action| match action {
            video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
                outputs,
                ..
            } => outputs.first(),
            _ => None,
        })
        .expect("completion output");
    assert_eq!(
        completion.media_id,
        "generated-google-veo-1-google-veo-output"
    );
    assert_eq!(
        completion.source_url,
        Some(format!("{base_url}/download/google-veo.mp4"))
    );
    assert_eq!(
        completion.relative_path,
        "generated/generated-google-veo-1/google-veo-output.mp4"
    );
    assert_eq!(completion.width, 1280);
    assert_eq!(completion.height, 720);
    assert_eq!(completion.duration_seconds, 8.0);
    assert_eq!(completion.fps, 24.0);

    let requests = request_handle.join().expect("Google Veo server");
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with(
        "POST /v1beta/models/veo-3.1-fast-generate-preview:predictLongRunning HTTP/1.1"
    ));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("x-goog-api-key: test-gemini-token"));
    assert!(requests[0].contains(r#""durationSeconds":8"#));
    assert!(requests[0].contains(r#""aspectRatio":"16:9""#));
    assert!(requests[0].contains(r#""generateAudio":true"#));
    assert!(requests[1].starts_with("GET /v1beta/operations/google-veo-operation-1 HTTP/1.1"));
    assert!(requests[1]
        .to_ascii_lowercase()
        .contains("x-goog-api-key: test-gemini-token"));
    assert!(requests[2].starts_with("GET /download/google-veo.mp4 HTTP/1.1"));
    assert!(requests[2]
        .to_ascii_lowercase()
        .contains("x-goog-api-key: test-gemini-token"));
}

#[test]
fn google_gemini_tts_asset_builds_interactions_audio_request() {
    let asset = generated_google_gemini_tts_asset(GeneratedAssetSettings {
        duration_seconds: Some(6.0),
        voice: Some("Kore".to_string()),
        category: Some("tts".to_string()),
        style_instructions: Some("calm documentary voice, close mic".to_string()),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_google_gemini_tts_generation_submission(&asset).expect("build Gemini TTS submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(
        submission.url,
        "https://generativelanguage.googleapis.com/v1beta/interactions"
    );
    assert_eq!(submission.model, GOOGLE_GEMINI_TTS_MODEL_ID);
    assert_eq!(submission.provider, GOOGLE_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "model": GOOGLE_GEMINI_TTS_MODEL_ID,
            "input": "calm documentary voice, close mic\n\nRead this concise product launch narration.",
            "response_format": {
                "type": "audio"
            },
            "generation_config": {
                "speech_config": [
                    { "voice": "Kore" }
                ]
            }
        })
    );
}

#[test]
fn google_gemini_tts_run_posts_json_decodes_base64_and_writes_wav_output() {
    let asset = generated_google_gemini_tts_asset(GeneratedAssetSettings {
        duration_seconds: Some(6.0),
        voice: Some("Puck".to_string()),
        category: Some("tts".to_string()),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_google_gemini_tts_generation_submission(&asset).expect("build Gemini TTS submission");
    let (base_url, request_handle) = spawn_google_gemini_tts_server();
    submission.url = format!("{base_url}/v1beta/interactions");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_google_gemini_tts_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("google-run-tts-1"),
            None,
        ),
        &submission,
        "test-gemini-token",
    )
    .expect("run Gemini TTS");

    assert_eq!(run.request_id, "google-generated-google-tts-1");
    let wav_bytes = std::fs::read(&run.completion.output_path).expect("Gemini TTS output");
    assert!(wav_bytes.starts_with(b"RIFF"));
    assert_eq!(&wav_bytes[8..12], b"WAVE");
    assert!(wav_bytes.ends_with(b"google-tts-pcm"));
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-google-tts-1/google-gemini-tts-output.wav")
    );
    let completion = run
        .completion
        .actions
        .iter()
        .find_map(|action| match action {
            video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
                outputs,
                ..
            } => outputs.first(),
            _ => None,
        })
        .expect("completion output");
    assert_eq!(
        completion.media_id,
        "generated-google-tts-1-google-gemini-tts-output"
    );
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-google-tts-1/google-gemini-tts-output.wav"
    );
    assert_eq!(completion.width, 1);
    assert_eq!(completion.height, 1);
    assert_eq!(completion.duration_seconds, 6.0);

    let requests = request_handle.join().expect("Gemini TTS server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1beta/interactions HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("x-goog-api-key: test-gemini-token"));
    assert!(requests[0].contains(r#""model":"gemini-3.1-flash-tts-preview""#));
    assert!(requests[0].contains(r#""response_format":{"type":"audio"}"#));
    assert!(requests[0].contains(r#""voice":"Puck""#));
}

#[test]
fn google_lyria_pro_asset_builds_interactions_music_request() {
    let asset = generated_google_lyria_asset(GeneratedAssetSettings {
        duration_seconds: Some(120.0),
        category: Some("music".to_string()),
        instrumental: Some(true),
        style_instructions: Some("bright, commercial, loopable".to_string()),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_google_lyria_generation_submission(&asset).expect("build Lyria submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(
        submission.url,
        "https://generativelanguage.googleapis.com/v1beta/interactions"
    );
    assert_eq!(submission.model, GOOGLE_LYRIA_3_PRO_MODEL_ID);
    assert_eq!(submission.api_model, GOOGLE_LYRIA_3_PRO_API_MODEL_ID);
    assert_eq!(submission.provider, GOOGLE_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "model": GOOGLE_LYRIA_3_PRO_API_MODEL_ID,
            "input": "Create a two minute cinematic synthwave song with a strong chorus.\n\nStyle: bright, commercial, loopable\n\nInstrumental only, no vocals.\n\nTarget duration: 120 seconds.",
            "response_format": {
                "type": "audio"
            }
        })
    );
}

#[test]
fn google_lyria_run_posts_json_decodes_base64_and_writes_mp3_output() {
    let asset = generated_google_lyria_asset(GeneratedAssetSettings {
        duration_seconds: Some(120.0),
        category: Some("music".to_string()),
        instrumental: Some(true),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_google_lyria_generation_submission(&asset).expect("build Lyria submission");
    let (base_url, request_handle) = spawn_google_lyria_server();
    submission.url = format!("{base_url}/v1beta/interactions");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_google_lyria_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("google-run-lyria-1"),
            None,
        ),
        &submission,
        "test-gemini-token",
    )
    .expect("run Lyria");

    assert_eq!(run.request_id, "google-generated-google-lyria-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("Lyria output"),
        b"google-lyria-mp3"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-google-lyria-1/google-lyria-output.mp3")
    );
    let completion = run
        .completion
        .actions
        .iter()
        .find_map(|action| match action {
            video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
                outputs,
                ..
            } => outputs.first(),
            _ => None,
        })
        .expect("completion output");
    assert_eq!(
        completion.media_id,
        "generated-google-lyria-1-google-lyria-output"
    );
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-google-lyria-1/google-lyria-output.mp3"
    );
    assert_eq!(completion.width, 1);
    assert_eq!(completion.height, 1);
    assert_eq!(completion.duration_seconds, 120.0);

    let requests = request_handle.join().expect("Lyria server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1beta/interactions HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("x-goog-api-key: test-gemini-token"));
    assert!(requests[0].contains(r#""model":"lyria-3-pro-preview""#));
    assert!(requests[0].contains(r#""response_format":{"type":"audio"}"#));
    assert!(requests[0].contains("Instrumental only, no vocals."));
}

#[test]
fn google_veo_poll_wait_is_interrupted_by_local_cancellation() {
    const PROJECT_ID: &str = "google-veo-cancel-project";
    const JOB_ID: &str = "generated-google-veo-1";
    let asset = generated_google_veo_asset(GeneratedAssetSettings::default());
    let mut submission =
        build_google_veo_generation_submission(&asset).expect("build Google Veo submission");
    let guard =
        register_generation_cancellation(PROJECT_ID, JOB_ID).expect("register cancellation");
    let token = guard.token();
    let (base_url, request_handle) =
        spawn_google_pending_veo_cancellation_server(PROJECT_ID, JOB_ID);
    submission.url =
        format!("{base_url}/v1beta/models/veo-3.1-fast-generate-preview:predictLongRunning");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let started = Instant::now();
    let error = run_google_veo_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("google-veo-cancel-run"),
            None,
        ),
        &submission,
        "test-gemini-token",
        Some(&token),
    )
    .expect_err("Google Veo should cancel");

    assert_eq!(error, GoogleGenerationWorkerError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!project_dir
        .path()
        .join("generated/generated-google-veo-1/google-veo-output.mp4")
        .exists());
    assert_eq!(
        request_handle
            .join()
            .expect("Google Veo cancellation server")
            .len(),
        2
    );
}

#[test]
fn google_tts_cancellation_after_response_writes_no_artifact() {
    const PROJECT_ID: &str = "google-tts-cancel-project";
    const JOB_ID: &str = "generated-google-tts-1";
    let asset = generated_google_gemini_tts_asset(GeneratedAssetSettings::default());
    let mut submission =
        build_google_gemini_tts_generation_submission(&asset).expect("build Google TTS submission");
    let guard =
        register_generation_cancellation(PROJECT_ID, JOB_ID).expect("register cancellation");
    let token = guard.token();
    let (base_url, request_handle) = spawn_google_tts_cancellation_server(PROJECT_ID, JOB_ID);
    submission.url = format!("{base_url}/v1beta/interactions");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let error = run_google_gemini_tts_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("google-tts-cancel-run"),
            None,
        ),
        &submission,
        "test-gemini-token",
        Some(&token),
    )
    .expect_err("Google TTS should cancel");

    assert_eq!(error, GoogleGenerationWorkerError::Cancelled);
    assert!(!project_dir
        .path()
        .join("generated/generated-google-tts-1/google-gemini-tts-output.wav")
        .exists());
    assert_eq!(
        request_handle
            .join()
            .expect("Google TTS cancellation server")
            .len(),
        1
    );
}

#[test]
fn google_lyria_pre_cancel_skips_network_and_artifact_creation() {
    const PROJECT_ID: &str = "google-lyria-cancel-project";
    const JOB_ID: &str = "generated-google-lyria-1";
    let asset = generated_google_lyria_asset(GeneratedAssetSettings::default());
    let submission =
        build_google_lyria_generation_submission(&asset).expect("build Google Lyria submission");
    let guard =
        register_generation_cancellation(PROJECT_ID, JOB_ID).expect("register cancellation");
    request_generation_cancellation(PROJECT_ID, JOB_ID);
    let token = guard.token();
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .expect("client");

    let error = run_google_lyria_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("google-lyria-cancel-run"),
            None,
        ),
        &submission,
        "test-gemini-token",
        Some(&token),
    )
    .expect_err("Google Lyria should be pre-cancelled");

    assert_eq!(error, GoogleGenerationWorkerError::Cancelled);
    assert!(!project_dir.path().join("generated").exists());
}

fn spawn_google_veo_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Google Veo server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle_base_url = base_url.clone();
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept Google Veo request");
            let mut buffer = [0_u8; 8192];
            let bytes_read = stream.read(&mut buffer).expect("read Google Veo request");
            let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
            requests.push(request);

            let response = match index {
                0 => response_json(r#"{"name":"operations/google-veo-operation-1"}"#),
                1 => response_json(&format!(
                    r#"{{
                        "name": "operations/google-veo-operation-1",
                        "done": true,
                        "response": {{
                            "generateVideoResponse": {{
                                "generatedSamples": [{{
                                    "video": {{
                                        "uri": "{handle_base_url}/download/google-veo.mp4"
                                    }}
                                }}]
                            }}
                        }}
                    }}"#
                )),
                _ => response_binary("video/mp4", b"google-veo-mp4"),
            };
            stream
                .write_all(&response)
                .expect("write Google Veo response");
        }
        requests
    });

    (base_url, handle)
}

fn spawn_google_gemini_tts_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Gemini TTS server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept Gemini TTS request");
        let mut buffer = [0_u8; 8192];
        let bytes_read = stream.read(&mut buffer).expect("read Gemini TTS request");
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
        let response = response_json(
            r#"{"output_audio":{"data":"Z29vZ2xlLXR0cy1wY20=","mime_type":"audio/pcm"}}"#,
        );
        stream
            .write_all(&response)
            .expect("write Gemini TTS response");
        vec![request]
    });

    (base_url, handle)
}

fn spawn_google_lyria_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Lyria server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept Lyria request");
        let mut buffer = [0_u8; 8192];
        let bytes_read = stream.read(&mut buffer).expect("read Lyria request");
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
        let response = response_json(
            r#"{"steps":[{"type":"model_output","content":[{"type":"text","text":"Generated song structure"},{"type":"audio","data":"Z29vZ2xlLWx5cmlhLW1wMw==","mime_type":"audio/mpeg"}]}]}"#,
        );
        stream.write_all(&response).expect("write Lyria response");
        vec![request]
    });

    (base_url, handle)
}

fn spawn_google_pending_veo_cancellation_server(
    project_id: &'static str,
    job_id: &'static str,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Google Veo pending server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener
                .accept()
                .expect("accept Google Veo pending request");
            let mut buffer = [0_u8; 8192];
            let bytes_read = stream
                .read(&mut buffer)
                .expect("read Google Veo pending request");
            requests.push(String::from_utf8_lossy(&buffer[..bytes_read]).to_string());
            let response = if index == 0 {
                response_json(r#"{"name":"operations/google-veo-cancel-operation"}"#)
            } else {
                response_json(r#"{"name":"operations/google-veo-cancel-operation","done":false}"#)
            };
            stream
                .write_all(&response)
                .expect("write Google Veo pending response");
            if index == 1 {
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(20));
                    request_generation_cancellation(project_id, job_id);
                });
            }
        }
        requests
    });
    (base_url, handle)
}

fn spawn_google_tts_cancellation_server(
    project_id: &'static str,
    job_id: &'static str,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Google TTS cancellation server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("accept Google TTS cancellation request");
        let mut buffer = [0_u8; 8192];
        let bytes_read = stream
            .read(&mut buffer)
            .expect("read Google TTS cancellation request");
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
        let response = response_json(
            r#"{"output_audio":{"data":"Z29vZ2xlLXR0cy1wY20=","mime_type":"audio/pcm"}}"#,
        );
        stream
            .write_all(&response)
            .expect("write Google TTS cancellation response");
        request_generation_cancellation(project_id, job_id);
        vec![request]
    });
    (base_url, handle)
}

fn response_json(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

fn response_binary(content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}
