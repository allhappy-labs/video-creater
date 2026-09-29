use reqwest::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
};
use video_creater_lib::generation::xai::{
    build_xai_image_generation_submission, build_xai_video_generation_submission,
    run_xai_image_generation_submission_with_client,
    run_xai_image_generation_submission_with_client_cancellable,
    run_xai_video_generation_submission_with_client,
    run_xai_video_generation_submission_with_client_cancellable,
    run_xai_video_generation_submission_with_client_reporting, XAiGenerationWorkerError,
    XAI_GROK_IMAGE_QUALITY_MODEL_ID, XAI_GROK_VIDEO_MODEL_ID, XAI_PROVIDER,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::job_progress::{read_job_progress_snapshots, JobProgressReporter};
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_xai_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-xai-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Grok still".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  cinematic product still with a clean background  ".to_string(),
        model: GenerationModel {
            provider: XAI_PROVIDER.to_string(),
            id: XAI_GROK_IMAGE_QUALITY_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_xai_edit_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    let mut asset = generated_xai_asset(settings);
    asset.id = "generated-xai-edit-1".to_string();
    asset.name = Some("Grok edited still".to_string());
    asset.prompt = "  render this product as a pencil sketch  ".to_string();
    asset.references.media_ids = vec!["reference-product".to_string()];
    asset.references.reference_image_media_refs = vec!["reference-product".to_string()];
    asset.references.provider_input_urls =
        vec!["data:image/png;base64,cmVmZXJlbmNlLXBuZw==".to_string()];
    asset
}

fn generated_xai_video_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    let mut asset = generated_xai_asset(settings);
    asset.id = "generated-xai-video-1".to_string();
    asset.name = Some("Grok video".to_string());
    asset.prompt = "  cinematic product launch video with a slow dolly move  ".to_string();
    asset.model = GenerationModel {
        provider: XAI_PROVIDER.to_string(),
        id: XAI_GROK_VIDEO_MODEL_ID.to_string(),
    };
    asset
}

#[test]
fn xai_grok_image_asset_builds_images_generation_request() {
    let asset = generated_xai_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        aspect_ratio: Some("16:9".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });

    let submission = build_xai_image_generation_submission(&asset).expect("build xAI submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.x.ai/v1/images/generations");
    assert_eq!(submission.model, XAI_GROK_IMAGE_QUALITY_MODEL_ID);
    assert_eq!(submission.provider, XAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "cinematic product still with a clean background",
            "aspect_ratio": "16:9",
            "response_format": "url",
            "n": 1
        })
    );
}

#[test]
fn xai_grok_image_edit_asset_builds_images_edit_request_with_data_url_image() {
    let asset = generated_xai_edit_asset(GeneratedAssetSettings {
        width: Some(1024),
        height: Some(1024),
        aspect_ratio: Some("1:1".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_xai_image_generation_submission(&asset).expect("build xAI edit submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.x.ai/v1/images/edits");
    assert_eq!(submission.model, XAI_GROK_IMAGE_QUALITY_MODEL_ID);
    assert_eq!(submission.provider, XAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "render this product as a pencil sketch",
            "image": {
                "url": "data:image/png;base64,cmVmZXJlbmNlLXBuZw==",
                "type": "image_url"
            },
            "aspect_ratio": "1:1",
            "response_format": "url",
            "n": 1
        })
    );
}

#[test]
fn xai_grok_video_asset_builds_async_video_generation_request() {
    let asset = generated_xai_video_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        duration_seconds: Some(6.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_xai_video_generation_submission(&asset).expect("build xAI video submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.x.ai/v1/videos/generations");
    assert_eq!(submission.model, XAI_GROK_VIDEO_MODEL_ID);
    assert_eq!(submission.provider, XAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "cinematic product launch video with a slow dolly move",
            "duration": 6,
            "aspect_ratio": "16:9",
            "resolution": "720p"
        })
    );
}

#[test]
fn xai_grok_video_asset_builds_reference_image_request() {
    let mut asset = generated_xai_video_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        duration_seconds: Some(10.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    asset.references.reference_image_media_refs =
        vec!["style-ref".to_string(), "product-ref".to_string()];
    asset.references.provider_input_urls = vec![
        "https://api.x.ai/v1/files/generated-xai-video/style.png".to_string(),
        "https://api.x.ai/v1/files/generated-xai-video/product.png".to_string(),
    ];

    let submission =
        build_xai_video_generation_submission(&asset).expect("build xAI video submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.x.ai/v1/videos/generations");
    assert_eq!(
        submission.input,
        json!({
            "prompt": "cinematic product launch video with a slow dolly move",
            "duration": 10,
            "aspect_ratio": "16:9",
            "resolution": "720p",
            "reference_images": [
                {"url": "https://api.x.ai/v1/files/generated-xai-video/style.png"},
                {"url": "https://api.x.ai/v1/files/generated-xai-video/product.png"}
            ]
        })
    );
}

#[test]
fn xai_grok_video_asset_builds_source_video_edit_request_without_text_video_settings() {
    let mut asset = generated_xai_video_asset(GeneratedAssetSettings {
        width: Some(1920),
        height: Some(1080),
        duration_seconds: Some(12.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    asset.references.source_video_media_ref = Some("source-video".to_string());
    asset.references.provider_input_urls =
        vec!["https://api.x.ai/v1/files/generated-xai-video/source.mp4".to_string()];

    let submission =
        build_xai_video_generation_submission(&asset).expect("build xAI video edit submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.x.ai/v1/videos/edits");
    assert_eq!(submission.model, XAI_GROK_VIDEO_MODEL_ID);
    assert_eq!(submission.provider, XAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "cinematic product launch video with a slow dolly move",
            "video": {
                "url": "https://api.x.ai/v1/files/generated-xai-video/source.mp4"
            }
        })
    );
}

#[test]
fn xai_image_generation_run_downloads_url_output_and_records_source_url() {
    let asset = generated_xai_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        aspect_ratio: Some("16:9".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    let mut submission = build_xai_image_generation_submission(&asset).expect("build xAI request");
    let (base_url, request_handle) = spawn_xai_image_server();
    submission.url = format!("{base_url}/v1/images/generations");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_xai_image_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("xai-run-1"),
            None,
        ),
        &submission,
        "test-xai-token",
    )
    .expect("run xAI generation");

    assert_eq!(run.request_id, "xai-generated-xai-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("xAI output"),
        b"xai-jpeg"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-xai-1/xai-output.jpg")
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
    assert_eq!(completion.media_id, "generated-xai-1-xai-output");
    assert_eq!(
        completion.source_url,
        Some(format!("{base_url}/generated/xai-output.jpg"))
    );
    assert_eq!(
        completion.relative_path,
        "generated/generated-xai-1/xai-output.jpg"
    );
    assert_eq!(completion.width, 1280);
    assert_eq!(completion.height, 720);

    let requests = request_handle.join().expect("xAI server");
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("POST /v1/images/generations HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("authorization: bearer test-xai-token"));
    assert!(requests[0].contains(r#""model":"grok-imagine-image-quality""#));
    assert!(requests[0].contains(r#""aspect_ratio":"16:9""#));
    assert!(requests[1].starts_with("GET /generated/xai-output.jpg HTTP/1.1"));
}

#[test]
fn xai_video_generation_run_polls_downloads_mp4_and_records_source_url() {
    let asset = generated_xai_video_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        duration_seconds: Some(6.0),
        fps: Some(24.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_xai_video_generation_submission(&asset).expect("build xAI video request");
    let (base_url, request_handle) = spawn_xai_video_server();
    submission.url = format!("{base_url}/v1/videos/generations");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_xai_video_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("xai-run-1"),
            None,
        ),
        &submission,
        "test-xai-token",
        2,
        Duration::from_millis(1),
    )
    .expect("run xAI video generation");

    assert_eq!(run.request_id, "xai-video-request-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("xAI video output"),
        b"xai-mp4"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-xai-video-1/xai-output.mp4")
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
    assert_eq!(completion.media_id, "generated-xai-video-1-xai-output");
    assert_eq!(
        completion.source_url,
        Some(format!("{base_url}/generated/xai-output.mp4"))
    );
    assert_eq!(
        completion.relative_path,
        "generated/generated-xai-video-1/xai-output.mp4"
    );
    assert_eq!(completion.width, 1280);
    assert_eq!(completion.height, 720);
    assert_eq!(completion.duration_seconds, 6.0);
    assert_eq!(completion.fps, 24.0);

    let requests = request_handle.join().expect("xAI video server");
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("POST /v1/videos/generations HTTP/1.1"));
    assert!(requests[0].contains(r#""model":"grok-imagine-video""#));
    assert!(requests[0].contains(r#""duration":6"#));
    assert!(requests[0].contains(r#""resolution":"720p""#));
    assert!(requests[1].starts_with("GET /v1/videos/xai-video-request-1 HTTP/1.1"));
    assert!(requests[2].starts_with("GET /generated/xai-output.mp4 HTTP/1.1"));
}

#[test]
fn xai_image_generation_cancellation_after_download_writes_no_artifact() {
    const PROJECT_ID: &str = "xai-cancel-project";
    const JOB_ID: &str = "generated-xai-1";
    let asset = generated_xai_asset(GeneratedAssetSettings::default());
    let mut submission = build_xai_image_generation_submission(&asset).expect("build xAI request");
    let guard =
        register_generation_cancellation(PROJECT_ID, JOB_ID).expect("register cancellation");
    let token = guard.token();
    let (base_url, request_handle) = spawn_xai_image_cancellation_server(PROJECT_ID, JOB_ID);
    submission.url = format!("{base_url}/v1/images/generations");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let error = run_xai_image_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("xai-cancel-run"),
            None,
        ),
        &submission,
        "test-xai-token",
        Some(&token),
    )
    .expect_err("xAI image generation should cancel");

    assert_eq!(error, XAiGenerationWorkerError::Cancelled);
    assert!(!project_dir
        .path()
        .join("generated/generated-xai-1/xai-output.jpg")
        .exists());
    assert_eq!(
        request_handle
            .join()
            .expect("xAI cancellation server")
            .len(),
        2
    );
}

#[test]
fn xai_video_poll_wait_is_interrupted_by_local_cancellation() {
    const PROJECT_ID: &str = "xai-video-cancel-project";
    const JOB_ID: &str = "generated-xai-video-1";
    let asset = generated_xai_video_asset(GeneratedAssetSettings::default());
    let mut submission =
        build_xai_video_generation_submission(&asset).expect("build xAI video request");
    let guard =
        register_generation_cancellation(PROJECT_ID, JOB_ID).expect("register cancellation");
    let token = guard.token();
    let (base_url, request_handle) =
        spawn_xai_pending_video_cancellation_server(PROJECT_ID, JOB_ID);
    submission.url = format!("{base_url}/v1/videos/generations");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let started = Instant::now();
    let error = run_xai_video_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("xai-video-cancel-run"),
            None,
        ),
        &submission,
        "test-xai-token",
        10,
        Duration::from_secs(5),
        Some(&token),
    )
    .expect_err("xAI video generation should cancel");

    assert_eq!(error, XAiGenerationWorkerError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!project_dir
        .path()
        .join("generated/generated-xai-video-1/xai-output.mp4")
        .exists());
    assert_eq!(
        request_handle
            .join()
            .expect("xAI cancellation server")
            .len(),
        2
    );
}

fn spawn_xai_image_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind xAI server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let response_url = format!("{base_url}/generated/xai-output.jpg");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept xAI request");
            let mut buffer = [0_u8; 8192];
            let bytes_read = stream.read(&mut buffer).expect("read xAI request");
            let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
            requests.push(request);
            if index == 0 {
                let body = format!(
                    r#"{{"data":[{{"url":"{response_url}","mime_type":"image/jpeg","revised_prompt":""}}],"usage":{{"cost_in_usd_ticks":200000000}}}}"#
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write xAI JSON response");
            } else {
                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: image/jpeg\r\n",
                    "Content-Length: 8\r\n",
                    "\r\n",
                    "xai-jpeg"
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write xAI image response");
            }
        }
        requests
    });

    (base_url, handle)
}

fn spawn_xai_video_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind xAI video server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let response_url = format!("{base_url}/generated/xai-output.mp4");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept xAI video request");
            let mut buffer = [0_u8; 8192];
            let bytes_read = stream.read(&mut buffer).expect("read xAI video request");
            let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
            requests.push(request);
            if index == 0 {
                let body = r#"{"request_id":"xai-video-request-1"}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write xAI video start response");
            } else if index == 1 {
                let body = format!(
                    r#"{{"status":"done","video":{{"url":"{response_url}","duration":6,"respect_moderation":true}},"model":"grok-imagine-video","progress":100}}"#
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write xAI video status response");
            } else {
                let response = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: video/mp4\r\n",
                    "Content-Length: 7\r\n",
                    "\r\n",
                    "xai-mp4"
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write xAI video bytes");
            }
        }
        requests
    });

    (base_url, handle)
}

fn spawn_xai_image_cancellation_server(
    project_id: &'static str,
    job_id: &'static str,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind xAI cancellation server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let response_url = format!("{base_url}/generated/xai-output.jpg");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept xAI cancellation request");
            let mut buffer = [0_u8; 8192];
            let bytes_read = stream
                .read(&mut buffer)
                .expect("read xAI cancellation request");
            requests.push(String::from_utf8_lossy(&buffer[..bytes_read]).to_string());
            let response = if index == 0 {
                xai_response_json(&format!(
                    r#"{{"data":[{{"url":"{response_url}","mime_type":"image/jpeg"}}]}}"#
                ))
            } else {
                xai_response_binary("image/jpeg", b"xai-jpeg")
            };
            stream
                .write_all(&response)
                .expect("write xAI cancellation response");
            if index == 1 {
                request_generation_cancellation(project_id, job_id);
            }
        }
        requests
    });
    (base_url, handle)
}

fn spawn_xai_pending_video_cancellation_server(
    project_id: &'static str,
    job_id: &'static str,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind xAI pending server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for index in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept xAI pending request");
            let mut buffer = [0_u8; 8192];
            let bytes_read = stream.read(&mut buffer).expect("read xAI pending request");
            requests.push(String::from_utf8_lossy(&buffer[..bytes_read]).to_string());
            let response = if index == 0 {
                xai_response_json(r#"{"request_id":"xai-video-cancel-request"}"#)
            } else {
                xai_response_json(r#"{"status":"pending","progress":10}"#)
            };
            stream
                .write_all(&response)
                .expect("write xAI pending response");
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

fn xai_response_json(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

fn xai_response_binary(content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

#[test]
fn xai_video_status_progress_is_reported_through_the_job_progress_reporter() {
    let asset = generated_xai_video_asset(GeneratedAssetSettings {
        width: Some(1280),
        height: Some(720),
        duration_seconds: Some(6.0),
        fps: Some(24.0),
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("720p".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_xai_video_generation_submission(&asset).expect("build xAI video request");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    std::fs::write(
        project_dir
            .path()
            .join(video_creater_lib::project::storage::PROJECT_FILE_NAME),
        b"{}",
    )
    .expect("project manifest");
    let (base_url, server) = spawn_xai_progress_video_server(project_dir.path().to_path_buf());
    submission.url = format!("{base_url}/v1/videos/generations");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");
    let reporter = JobProgressReporter::new(project_dir.path(), "job-xai-video-1");

    run_xai_video_generation_submission_with_client_reporting(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("xai-run-1"),
            None,
        ),
        &submission,
        "test-xai-token",
        4,
        // Longer than the reporter's 500 ms throttle, so both pending polls are written.
        Duration::from_millis(600),
        None,
        Some(&reporter),
    )
    .expect("run xAI video generation");

    let observed_before_completion = server.join().expect("xAI progress server");
    assert_eq!(observed_before_completion, Some(0.55));
}

/// Answers start, pending 10 %, pending 55 %, done, then the video bytes. Before answering "done"
/// it reads the job's progress snapshot.
fn spawn_xai_progress_video_server(
    project_dir: std::path::PathBuf,
) -> (String, thread::JoinHandle<Option<f64>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind xAI progress server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let response_url = format!("{base_url}/generated/xai-output.mp4");
    let handle = thread::spawn(move || {
        let mut observed = None;
        for index in 0..5 {
            let (mut stream, _) = listener.accept().expect("accept xAI progress request");
            let mut buffer = [0_u8; 8192];
            let _ = stream.read(&mut buffer).expect("read xAI progress request");
            let response = match index {
                0 => xai_response_json(r#"{"request_id":"xai-video-request-1"}"#),
                1 => xai_response_json(r#"{"status":"pending","progress":10}"#),
                2 => xai_response_json(r#"{"status":"pending","progress":55}"#),
                3 => {
                    observed = read_job_progress_snapshots(&project_dir)
                        .expect("read progress")
                        .into_iter()
                        .find(|snapshot| snapshot.job_id == "job-xai-video-1")
                        .map(|snapshot| snapshot.progress);
                    xai_response_json(&format!(
                        r#"{{"status":"done","video":{{"url":"{response_url}","duration":6}},"progress":100}}"#
                    ))
                }
                _ => xai_response_binary("video/mp4", b"xai-mp4"),
            };
            stream
                .write_all(&response)
                .expect("write xAI progress response");
        }
        observed
    });
    (base_url, handle)
}
