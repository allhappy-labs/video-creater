use reqwest::blocking::Client;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;
use video_creater_lib::generation::minimax::{
    build_minimax_generation_submission, run_minimax_generation_submission_with_client,
    MINIMAX_MUSIC_API_MODEL_ID, MINIMAX_MUSIC_MODEL_ID, MINIMAX_PROVIDER,
};
use video_creater_lib::generation::GenerationTarget;
use video_creater_lib::project::model::{
    GeneratedAsset, GeneratedAssetReferences, GeneratedAssetSettings, GeneratedAssetStatus,
    GenerationModel, MediaKind,
};

fn generated_minimax_music_asset(settings: GeneratedAssetSettings) -> GeneratedAsset {
    GeneratedAsset {
        schema_version: 1,
        id: "generated-minimax-music-1".to_string(),
        kind: MediaKind::Generated,
        status: GeneratedAssetStatus::Queued,
        name: Some("MiniMax music bed".to_string()),
        target_folder_id: None,
        placement_intent: None,
        prompt: "  Bright indie pop, upbeat launch video, 112 BPM.  ".to_string(),
        model: GenerationModel {
            provider: MINIMAX_PROVIDER.to_string(),
            id: MINIMAX_MUSIC_MODEL_ID.to_string(),
        },
        references: GeneratedAssetReferences::default(),
        settings,
        outputs: Vec::new(),
        created_at: "2026-07-07T12:00:00Z".to_string(),
        parent_asset_id: None,
        retry_of_asset_id: None,
    }
}

#[test]
fn minimax_music_asset_builds_music_generation_request_with_lyrics() {
    let asset = generated_minimax_music_asset(GeneratedAssetSettings {
        category: Some("music".to_string()),
        lyrics: Some("[Verse]\nLaunch lights rising\n[Chorus]\nWe move as one".to_string()),
        instrumental: Some(false),
        style_instructions: Some("glossy drums, bright hook".to_string()),
        duration_seconds: Some(180.0),
        ..GeneratedAssetSettings::default()
    });

    let submission = build_minimax_generation_submission(&asset).expect("build MiniMax request");

    assert_eq!(submission.method, "POST");
    assert_eq!(submission.url, "https://api.minimax.io/v1/music_generation");
    assert_eq!(submission.model, MINIMAX_MUSIC_MODEL_ID);
    assert_eq!(submission.api_model, MINIMAX_MUSIC_API_MODEL_ID);
    assert_eq!(submission.provider, MINIMAX_PROVIDER);
    assert!(serde_json::to_value(&submission)
        .unwrap()
        .get("authEnvVar")
        .is_none());
    assert_eq!(
        submission.input,
        json!({
            "model": "music-2.6",
            "prompt": "Bright indie pop, upbeat launch video, 112 BPM.\n\nStyle: glossy drums, bright hook",
            "lyrics": "[Verse]\nLaunch lights rising\n[Chorus]\nWe move as one",
            "output_format": "hex",
            "lyrics_optimizer": false,
            "is_instrumental": false,
            "audio_setting": {
                "sample_rate": 44100,
                "bitrate": 256000,
                "format": "mp3"
            }
        })
    );
}

#[test]
fn minimax_music_without_lyrics_enables_lyrics_optimizer() {
    let asset = generated_minimax_music_asset(GeneratedAssetSettings {
        category: Some("music".to_string()),
        instrumental: Some(false),
        ..GeneratedAssetSettings::default()
    });

    let submission = build_minimax_generation_submission(&asset).expect("build MiniMax request");

    assert_eq!(
        submission.input,
        json!({
            "model": "music-2.6",
            "prompt": "Bright indie pop, upbeat launch video, 112 BPM.",
            "output_format": "hex",
            "lyrics_optimizer": true,
            "is_instrumental": false,
            "audio_setting": {
                "sample_rate": 44100,
                "bitrate": 256000,
                "format": "mp3"
            }
        })
    );
}

#[test]
fn minimax_music_run_posts_json_decodes_hex_and_writes_mp3_output() {
    let asset = generated_minimax_music_asset(GeneratedAssetSettings {
        category: Some("music".to_string()),
        lyrics: Some("[Verse]\nLaunch lights rising".to_string()),
        instrumental: Some(false),
        ..GeneratedAssetSettings::default()
    });
    let mut submission =
        build_minimax_generation_submission(&asset).expect("build MiniMax request");
    let (base_url, request_handle) = spawn_minimax_music_server();
    submission.url = format!("{base_url}/v1/music_generation");
    let project_dir = tempfile::tempdir().expect("temp project dir");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client");

    let run = run_minimax_generation_submission_with_client(
        &client,
        GenerationTarget::new(
            project_dir.path(),
            &asset,
            "2026-07-07T12:01:00Z",
            Some("minimax-run-music-1"),
            None,
        ),
        &submission,
        "test-minimax-token",
    )
    .expect("run MiniMax music");

    assert_eq!(run.request_id, "minimax-generated-minimax-music-1");
    assert_eq!(
        std::fs::read(&run.completion.output_path).expect("MiniMax music output"),
        b"minimax-mp3"
    );
    assert_eq!(
        run.completion.output_path,
        project_dir
            .path()
            .join("generated/generated-minimax-music-1/minimax-music-output.mp3")
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
        "generated-minimax-music-1-minimax-music-output"
    );
    assert_eq!(completion.source_url, None);
    assert_eq!(
        completion.relative_path,
        "generated/generated-minimax-music-1/minimax-music-output.mp3"
    );
    assert_eq!(completion.width, 0);
    assert_eq!(completion.height, 0);
    assert_eq!(completion.duration_seconds, 25.364);
    assert_eq!(completion.fps, 0.0);

    let requests = request_handle.join().expect("MiniMax music server");
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/music_generation HTTP/1.1"));
    assert!(requests[0]
        .to_ascii_lowercase()
        .contains("authorization: bearer test-minimax-token"));
    assert!(requests[0].contains(r#""model":"music-2.6""#));
    assert!(requests[0].contains(r#""output_format":"hex""#));
}

fn spawn_minimax_music_server() -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind MiniMax test server");
    let addr = listener.local_addr().expect("MiniMax server addr");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        let (mut stream, _) = listener.accept().expect("accept MiniMax request");
        let mut buffer = [0_u8; 8192];
        let read = stream.read(&mut buffer).expect("read MiniMax request");
        requests.push(String::from_utf8_lossy(&buffer[..read]).to_string());
        let body = r#"{"data":{"audio":"6d696e696d61782d6d7033","status":2},"extra_info":{"music_duration":25364,"music_sample_rate":44100,"music_channel":2,"bitrate":256000},"base_resp":{"status_code":0,"status_msg":"success"}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write MiniMax response");
        requests
    });

    (format!("http://{addr}"), handle)
}
