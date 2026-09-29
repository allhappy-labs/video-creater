use reqwest::blocking::Client;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::cancel::{
    register_generation_cancellation, request_generation_cancellation,
};
use video_creater_lib::generation::elevenlabs::{
    build_elevenlabs_generation_submission,
    run_elevenlabs_generation_submission_with_client_cancellable, ElevenLabsGenerationWorkerError,
    ELEVENLABS_PROVIDER, ELEVENLABS_TTS_V3_MODEL_ID,
};
use video_creater_lib::generation::minimax::{
    build_minimax_generation_submission, run_minimax_generation_submission_with_client_cancellable,
    MinimaxGenerationWorkerError, MINIMAX_MUSIC_MODEL_ID, MINIMAX_PROVIDER,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_audio_asset(id: &str, provider: &str, model_id: &str) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: id.to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("Generated audio".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "A concise production-ready audio prompt".to_string(),
        model: GenerationModel {
            provider: provider.to_string(),
            id: model_id.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings: GeneratedAssetSettings {
            duration_seconds: Some(4.0),
            ..GeneratedAssetSettings::default()
        },
        outputs: Vec::new(),
        created_at: "2026-07-12T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

#[test]
fn elevenlabs_cancellation_during_streaming_response_writes_no_artifact() {
    let asset = generated_audio_asset(
        "generated-elevenlabs-cancel",
        ELEVENLABS_PROVIDER,
        ELEVENLABS_TTS_V3_MODEL_ID,
    );
    let mut submission =
        build_elevenlabs_generation_submission(&asset).expect("build ElevenLabs submission");
    let project_id = "project-elevenlabs-stream-cancel";
    let guard = register_generation_cancellation(project_id, &asset.id)
        .expect("register ElevenLabs cancellation");
    let token = guard.token();
    let body = vec![b'x'; 128 * 1024];
    let (base_url, server) =
        spawn_streaming_cancellation_server(project_id, &asset.id, "audio/mpeg", body);
    submission.url = format!("{base_url}/v1/text-to-speech/test");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let error = run_elevenlabs_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-12T12:01:00Z",
            Some("elevenlabs-run-cancel"),
            None,
        ),
        &submission,
        "test-elevenlabs-token",
        Some(&token),
    )
    .expect_err("cancel ElevenLabs streaming response");

    assert_eq!(error, ElevenLabsGenerationWorkerError::Cancelled);
    assert!(!project_dir
        .path()
        .join("generated/generated-elevenlabs-cancel/elevenlabs-output.mp3")
        .exists());
    server.join().expect("ElevenLabs cancellation server");
}

#[test]
fn minimax_cancellation_during_streaming_json_response_writes_no_artifact() {
    let asset = generated_audio_asset(
        "generated-minimax-cancel",
        MINIMAX_PROVIDER,
        MINIMAX_MUSIC_MODEL_ID,
    );
    let mut submission =
        build_minimax_generation_submission(&asset).expect("build MiniMax submission");
    let project_id = "project-minimax-stream-cancel";
    let guard = register_generation_cancellation(project_id, &asset.id)
        .expect("register MiniMax cancellation");
    let token = guard.token();
    let audio_hex = "00".repeat(64 * 1024);
    let body = format!(r#"{{"data":{{"audio":"{audio_hex}"}},"base_resp":{{"status_code":0}}}}"#)
        .into_bytes();
    let (base_url, server) =
        spawn_streaming_cancellation_server(project_id, &asset.id, "application/json", body);
    submission.url = format!("{base_url}/v1/music_generation");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let error = run_minimax_generation_submission_with_client_cancellable(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-12T12:01:00Z",
            Some("minimax-run-cancel"),
            None,
        ),
        &submission,
        "test-minimax-token",
        Some(&token),
    )
    .expect_err("cancel MiniMax streaming response");

    assert_eq!(error, MinimaxGenerationWorkerError::Cancelled);
    assert!(!project_dir
        .path()
        .join("generated/generated-minimax-cancel/minimax-output.mp3")
        .exists());
    server.join().expect("MiniMax cancellation server");
}

fn spawn_streaming_cancellation_server(
    project_id: &'static str,
    job_id: &str,
    content_type: &'static str,
    body: Vec<u8>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind streaming cancellation server");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let job_id = job_id.to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept generation request");
        let mut request = [0_u8; 8192];
        let _ = stream.read(&mut request).expect("read generation request");
        let headers = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\r\n",
            body.len(),
        );
        stream.write_all(headers.as_bytes()).expect("write headers");
        let split = body.len().min(64 * 1024);
        stream.write_all(&body[..split]).expect("write first chunk");
        stream.flush().expect("flush first chunk");
        request_generation_cancellation(project_id, &job_id);
        thread::sleep(Duration::from_millis(50));
        let _ = stream.write_all(&body[split..]);
    });
    (base_url, handle)
}
