use reqwest::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
};
use video_creater_lib::generation::openai::{
    build_openai_image_generation_submission, run_openai_image_generation_submission_with_client,
    run_openai_image_generation_submission_with_client_cancellable, OpenAiGenerationWorkerError,
    OPENAI_GPT_4O_MINI_TTS_MODEL_ID, OPENAI_GPT_IMAGE_2_MODEL_ID, OPENAI_GPT_IMAGE_EDIT_MODEL_ID,
    OPENAI_PROVIDER,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_openai_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-openai-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("OpenAI still".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  editorial product still with natural shadows  ".to_string(),
        model: GenerationModel {
            provider: OPENAI_PROVIDER.to_string(),
            id: OPENAI_GPT_IMAGE_2_MODEL_ID.to_string(),
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
        created_at: "2026-07-06T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

fn generated_openai_edit_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    let mut asset = generated_openai_asset(settings);
    asset.id = "generated-openai-edit-1".to_string();
    asset.name = Some("OpenAI edited still".to_string());
    asset.prompt = "  keep the same product but change the set styling  ".to_string();
    asset.model = GenerationModel {
        provider: OPENAI_PROVIDER.to_string(),
        id: OPENAI_GPT_IMAGE_EDIT_MODEL_ID.to_string(),
    };
    asset.references.reference_image_media_refs = vec!["reference-style".to_string()];
    asset.references.media_ids = vec!["reference-style".to_string()];
    asset.references.provider_input_urls =
        vec!["data:image/png;base64,cmVmZXJlbmNlLXBuZw==".to_string()];
    asset
}

fn generated_openai_tts_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    let mut asset = generated_openai_asset(settings);
    asset.id = "generated-openai-tts-1".to_string();
    asset.name = Some("OpenAI voiceover".to_string());
    asset.prompt = "  Read this concise product launch narration.  ".to_string();
    asset.model = GenerationModel {
        provider: OPENAI_PROVIDER.to_string(),
        id: OPENAI_GPT_4O_MINI_TTS_MODEL_ID.to_string(),
    };
    asset
}

#[test]
fn openai_gpt_image_2_asset_builds_images_api_request() {
    let asset = generated_openai_asset(GeneratedAssetSettings {
        width: Some(1536),
        height: Some(864),
        duration_seconds: None,
        fps: None,
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("1536x864".to_string()),
        quality: Some("high".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(
        submission.url,
        "https://api.openai.com/v1/images/generations"
    );
    assert_eq!(submission.model, OPENAI_GPT_IMAGE_2_MODEL_ID);
    assert_eq!(submission.provider, OPENAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "editorial product still with natural shadows",
            "size": "1536x864",
            "quality": "high",
            "output_format": "png",
            "n": 1
        })
    );
}

#[test]
fn openai_gpt_image_2_asset_serializes_requested_image_count() {
    let asset = generated_openai_asset(GeneratedAssetSettings {
        width: Some(1024),
        height: Some(1024),
        duration_seconds: None,
        fps: None,
        aspect_ratio: Some("1:1".to_string()),
        resolution: Some("1024x1024".to_string()),
        num_images: Some(2),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI submission");

    assert_eq!(submission.input["n"], json!(2));
}

#[test]
fn openai_image_edit_asset_builds_images_edits_request_with_data_url_images() {
    let asset = generated_openai_edit_asset(GeneratedAssetSettings {
        width: Some(1024),
        height: Some(1024),
        duration_seconds: None,
        fps: None,
        aspect_ratio: Some("1:1".to_string()),
        resolution: Some("1024x1024".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI edit submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.openai.com/v1/images/edits");
    assert_eq!(submission.model, OPENAI_GPT_IMAGE_EDIT_MODEL_ID);
    assert_eq!(submission.provider, OPENAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "prompt": "keep the same product but change the set styling",
            "images": [
                { "image_url": "data:image/png;base64,cmVmZXJlbmNlLXBuZw==" }
            ],
            "size": "1024x1024",
            "quality": "auto",
            "output_format": "png",
            "n": 1
        })
    );
}

#[test]
fn openai_tts_asset_builds_audio_speech_request_with_voice_settings() {
    let asset = generated_openai_tts_asset(GeneratedAssetSettings {
        duration_seconds: Some(4.0),
        voice: Some("marin".to_string()),
        style_instructions: Some("calm documentary voice, close mic".to_string()),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI TTS submission");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.openai.com/v1/audio/speech");
    assert_eq!(submission.model, OPENAI_GPT_4O_MINI_TTS_MODEL_ID);
    assert_eq!(submission.provider, OPENAI_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "input": "Read this concise product launch narration.",
            "voice": "marin",
            "instructions": "calm documentary voice, close mic",
            "response_format": "mp3"
        })
    );
}

#[test]
fn openai_tts_asset_defaults_to_catalog_voice() {
    let asset = generated_openai_tts_asset(GeneratedAssetSettings {
        duration_seconds: Some(4.0),
        ..GeneratedAssetSettings::default()
    });

    let submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI TTS submission");

    assert_eq!(submission.input["voice"], json!("alloy"));
}

#[test]
fn openai_image_generation_run_writes_base64_output_without_provider_url() {
    let asset = generated_openai_asset(GeneratedAssetSettings {
        width: Some(1536),
        height: Some(864),
        duration_seconds: None,
        fps: None,
        aspect_ratio: Some("16:9".to_string()),
        resolution: Some("1536x864".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI submission");
    let (base_url, request_handle) = spawn_openai_image_server();
    submission.url = format!("{base_url}/v1/images/generations");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_openai_image_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("openai-run-1"),
            None,
        ),
        &submission,
        "test-openai-token",
    )
    .expect("run OpenAI generation");

    assert_eq!(run.request_id, "imggen-test-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("OpenAI output"),
        b"openai-png"
    );
    let output_path = project_dir
        .path()
        .join("generated/generated-openai-1/openai-output.png");
    assert_eq!(run.completion.output_path, output_path);
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
    assert_eq!(completion.media_id, "generated-openai-1-openai-output");
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-openai-1/openai-output.png"
    );
    assert_eq!(completion.width, 1536);
    assert_eq!(completion.height, 864);

    let requests = request_handle.join().expect("OpenAI server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/images/generations HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("authorization: bearer test-openai-token"));
    assert!(requests[0].contains(r#""model":"gpt-image-2""#));
    assert!(requests[0].contains(r#""size":"1536x864""#));
}

#[test]
fn openai_image_generation_run_retains_multiple_outputs() {
    let asset = generated_openai_asset(GeneratedAssetSettings {
        width: Some(1024),
        height: Some(1024),
        duration_seconds: None,
        fps: None,
        aspect_ratio: Some("1:1".to_string()),
        resolution: Some("1024x1024".to_string()),
        num_images: Some(2),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI submission");
    let (base_url, request_handle) = spawn_openai_multi_image_server();
    submission.url = format!("{base_url}/v1/images/generations");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_openai_image_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("openai-run-1"),
            Some("timeline-item-that-needs-explicit-output-selection"),
        ),
        &submission,
        "test-openai-token",
    )
    .expect("run OpenAI generation");

    assert_eq!(
        std::fs::read(
            project_dir
                .path()
                .join("generated/generated-openai-1/openai-output.png")
        )
        .expect("first OpenAI output"),
        b"openai-png"
    );
    assert_eq!(
        std::fs::read(
            project_dir
                .path()
                .join("generated/generated-openai-1/openai-output-2.png")
        )
        .expect("second OpenAI output"),
        b"openai-png-two"
    );
    let outputs = run
        .completion
        .actions
        .iter()
        .find_map(|action| match action {
            video_creater_lib::project::action::ProjectAction::CompleteGeneratedAsset {
                outputs,
                replacement,
                ..
            } => {
                assert_eq!(replacement, &None);
                Some(outputs)
            }
            _ => None,
        })
        .expect("completion outputs");
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0].media_id, "generated-openai-1-openai-output");
    assert_eq!(
        outputs[0].relative_path,
        "generated/generated-openai-1/openai-output.png"
    );
    assert_eq!(outputs[1].media_id, "generated-openai-1-openai-output-2");
    assert_eq!(
        outputs[1].relative_path,
        "generated/generated-openai-1/openai-output-2.png"
    );

    let requests = request_handle.join().expect("OpenAI server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].contains(r#""n":2"#));
}

#[test]
fn openai_image_edit_run_posts_images_edits_json_and_writes_output() {
    let asset = generated_openai_edit_asset(GeneratedAssetSettings {
        width: Some(1024),
        height: Some(1024),
        duration_seconds: None,
        fps: None,
        aspect_ratio: Some("1:1".to_string()),
        resolution: Some("1024x1024".to_string()),
        generate_audio: None,
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI edit submission");
    let (base_url, request_handle) = spawn_openai_image_server();
    submission.url = format!("{base_url}/v1/images/edits");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_openai_image_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("openai-run-1"),
            None,
        ),
        &submission,
        "test-openai-token",
    )
    .expect("run OpenAI edit");

    assert_eq!(run.request_id, "imggen-test-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("OpenAI edit output"),
        b"openai-png"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-openai-edit-1/openai-output.png")
    );

    let requests = request_handle.join().expect("OpenAI server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/images/edits HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("authorization: bearer test-openai-token"));
    assert!(requests[0].contains(r#""model":"gpt-image-1.5""#));
    assert!(requests[0].contains(r#""image_url":"data:image/png;base64,cmVmZXJlbmNlLXBuZw==""#));
}

#[test]
fn openai_tts_run_posts_audio_speech_json_and_writes_mp3_output() {
    let asset = generated_openai_tts_asset(GeneratedAssetSettings {
        duration_seconds: Some(4.0),
        voice: Some("cedar".to_string()),
        style_instructions: Some("warm narration".to_string()),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI TTS submission");
    let (base_url, request_handle) = spawn_openai_speech_server();
    submission.url = format!("{base_url}/v1/audio/speech");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_openai_image_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("openai-run-tts-1"),
            None,
        ),
        &submission,
        "test-openai-token",
    )
    .expect("run OpenAI TTS");

    assert_eq!(run.request_id, "openai-generated-openai-tts-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("OpenAI TTS output"),
        b"openai-mp3"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-openai-tts-1/openai-output.mp3")
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
    assert_eq!(completion.media_id, "generated-openai-tts-1-openai-output");
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-openai-tts-1/openai-output.mp3"
    );
    assert_eq!(completion.width, 1);
    assert_eq!(completion.height, 1);
    assert_eq!(completion.duration_seconds, 4.0);

    let requests = request_handle.join().expect("OpenAI speech server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/audio/speech HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("authorization: bearer test-openai-token"));
    assert!(requests[0].contains(r#""model":"gpt-4o-mini-tts""#));
    assert!(requests[0].contains(r#""voice":"cedar""#));
    assert!(requests[0].contains(r#""instructions":"warm narration""#));
}

#[test]
fn openai_tts_cancellation_during_streaming_response_writes_no_artifact() {
    let asset = generated_openai_tts_asset(GeneratedAssetSettings {
        duration_seconds: Some(4.0),
        voice: Some("cedar".to_string()),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_openai_image_generation_submission(&asset).expect("build OpenAI TTS submission");
    let project_id = "project-openai-stream-cancel";
    let guard = register_generation_cancellation(project_id, &asset.id)
        .expect("register OpenAI cancellation");
    let token = guard.token();
    let (base_url, server) = spawn_openai_streaming_cancellation_server(project_id, &asset.id);
    submission.url = format!("{base_url}/v1/audio/speech");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let error = run_openai_image_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-06T12:01:00Z",
            Some("openai-run-tts-cancel"),
            None,
        ),
        &submission,
        "test-openai-token",
        Some(&token),
    )
    .expect_err("cancel OpenAI streaming response");

    assert_eq!(error, OpenAiGenerationWorkerError::Cancelled);
    assert!(!project_dir
        .path()
        .join("generated/generated-openai-tts-1/openai-output.mp3")
        .exists());
    server.join().expect("OpenAI cancellation server");
}

fn spawn_openai_image_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OpenAI server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept OpenAI request");
        let mut request = String::new();
        let mut buffer = [0_u8; 4096];
        let bytes_read = stream.read(&mut buffer).expect("read OpenAI request");
        request.push_str(&String::from_utf8_lossy(&buffer[..bytes_read]));
        let body = r#"{"id":"imggen-test-1","data":[{"b64_json":"b3BlbmFpLXBuZw=="}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write OpenAI response");
        vec![request]
    });

    (base_url, handle)
}

fn spawn_openai_multi_image_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OpenAI server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept OpenAI request");
        let mut request = String::new();
        let mut buffer = [0_u8; 4096];
        let bytes_read = stream.read(&mut buffer).expect("read OpenAI request");
        request.push_str(&String::from_utf8_lossy(&buffer[..bytes_read]));
        let body = concat!(
            r#"{"id":"imggen-test-1","data":["#,
            r#"{"b64_json":"b3BlbmFpLXBuZw=="},"#,
            r#"{"b64_json":"b3BlbmFpLXBuZy10d28="}"#,
            r#"]}"#
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write OpenAI response");
        vec![request]
    });

    (base_url, handle)
}

fn spawn_openai_speech_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OpenAI speech server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept OpenAI speech request");
        let mut buffer = [0_u8; 8192];
        let bytes_read = stream
            .read(&mut buffer)
            .expect("read OpenAI speech request");
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
        let response = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: audio/mpeg\r\n",
            "Content-Length: 10\r\n",
            "\r\n",
            "openai-mp3"
        );
        stream
            .write_all(response.as_bytes())
            .expect("write OpenAI speech response");
        vec![request]
    });

    (base_url, handle)
}

fn spawn_openai_streaming_cancellation_server(
    project_id: &'static str,
    job_id: &str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OpenAI cancellation server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let job_id = job_id.to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .expect("accept OpenAI cancellation request");
        let mut buffer = [0_u8; 8192];
        let _ = stream
            .read(&mut buffer)
            .expect("read OpenAI cancellation request");
        let chunk = vec![b'x'; 64 * 1024];
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nContent-Length: {}\r\n\r\n",
            chunk.len() * 2,
        );
        stream.write_all(headers.as_bytes()).expect("write headers");
        stream.write_all(&chunk).expect("write first chunk");
        stream.flush().expect("flush first chunk");
        request_generation_cancellation(project_id, &job_id);
        thread::sleep(Duration::from_millis(50));
        let _ = stream.write_all(&chunk);
    });
    (base_url, handle)
}
